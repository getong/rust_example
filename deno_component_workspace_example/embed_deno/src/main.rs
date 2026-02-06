use std::{
  collections::BTreeMap,
  env,
  path::{Path, PathBuf},
  process::{Command, Output},
  rc::Rc,
  time::Duration,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use deno_ast::{EmitOptions, MediaType, ParseParams, SourceMapOption};
use deno_core::{
  JsRuntime, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode,
  ModuleSpecifier, ModuleType, ResolutionKind, RuntimeOptions,
  error::{AnyError, ModuleLoaderError},
  futures::FutureExt,
  op2, resolve_import,
};
use deno_graph::packages::{JsrPackageInfo, JsrPackageVersionInfo};
use deno_semver::jsr::JsrPackageReqReference;
use rand::RngCore;
use sha2::{Digest, Sha256};

#[op2(fast)]
fn op_get_random_values(#[buffer] output: &mut [u8]) {
  rand::thread_rng().fill_bytes(output);
}

#[op2]
async fn op_sleep(ms: u32) {
  tokio::time::sleep(Duration::from_millis(ms as u64)).await;
}

#[op2]
#[serde]
fn op_env_snapshot() -> BTreeMap<String, String> {
  env::vars().collect()
}

fn hmac_sha256(secret: &[u8], message: &[u8]) -> [u8; 32] {
  const BLOCK_SIZE: usize = 64;
  let mut key_block = [0u8; BLOCK_SIZE];

  if secret.len() > BLOCK_SIZE {
    let digest = Sha256::digest(secret);
    key_block[.. digest.len()].copy_from_slice(&digest);
  } else {
    key_block[.. secret.len()].copy_from_slice(secret);
  }

  let mut inner_pad = [0x36u8; BLOCK_SIZE];
  let mut outer_pad = [0x5cu8; BLOCK_SIZE];
  for index in 0 .. BLOCK_SIZE {
    inner_pad[index] ^= key_block[index];
    outer_pad[index] ^= key_block[index];
  }

  let mut inner = Sha256::new();
  inner.update(inner_pad);
  inner.update(message);
  let inner_digest = inner.finalize();

  let mut outer = Sha256::new();
  outer.update(outer_pad);
  outer.update(inner_digest);
  let digest = outer.finalize();

  let mut output = [0u8; 32];
  output.copy_from_slice(&digest);
  output
}

#[op2]
#[string]
fn op_sign_jwt_hs256(
  #[string] secret: String,
  #[serde] payload: deno_core::serde_json::Value,
  no_timestamp: bool,
) -> String {
  let mut payload = match payload {
    deno_core::serde_json::Value::Object(map) => map,
    _ => deno_core::serde_json::Map::new(),
  };

  if !no_timestamp && !payload.contains_key("iat") {
    let now = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs();
    payload.insert("iat".to_string(), deno_core::serde_json::Value::from(now));
  }

  let header_json = deno_core::serde_json::json!({
    "alg": "HS256",
    "typ": "JWT",
  })
  .to_string();
  let payload_json = deno_core::serde_json::Value::Object(payload).to_string();

  let header_b64 = URL_SAFE_NO_PAD.encode(header_json.as_bytes());
  let payload_b64 = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
  let signing_input = format!("{header_b64}.{payload_b64}");

  let signature = hmac_sha256(secret.as_bytes(), signing_input.as_bytes());
  let signature_b64 = URL_SAFE_NO_PAD.encode(signature);

  format!("{signing_input}.{signature_b64}")
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ReadTextFileSyncResult {
  ok: bool,
  text: Option<String>,
  error_kind: Option<&'static str>,
  error_message: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FetchHttpResult {
  status: u16,
  status_text: String,
  body: String,
  error: Option<String>,
}

fn http_status_text(status: u16) -> &'static str {
  match status {
    200 => "OK",
    201 => "Created",
    202 => "Accepted",
    204 => "No Content",
    301 => "Moved Permanently",
    302 => "Found",
    304 => "Not Modified",
    307 => "Temporary Redirect",
    308 => "Permanent Redirect",
    400 => "Bad Request",
    401 => "Unauthorized",
    403 => "Forbidden",
    404 => "Not Found",
    405 => "Method Not Allowed",
    408 => "Request Timeout",
    409 => "Conflict",
    413 => "Payload Too Large",
    415 => "Unsupported Media Type",
    429 => "Too Many Requests",
    500 => "Internal Server Error",
    502 => "Bad Gateway",
    503 => "Service Unavailable",
    504 => "Gateway Timeout",
    _ => "",
  }
}

fn run_curl_fetch(
  url: &str,
  method: &str,
  headers: &BTreeMap<String, String>,
  body: &Option<String>,
  disable_proxy_env: bool,
) -> std::io::Result<Output> {
  let mut command = Command::new("curl");
  command
    .arg("-sSL")
    .arg("-X")
    .arg(method)
    .arg("--connect-timeout")
    .arg("10")
    .arg("--max-time")
    .arg("60")
    .arg("--retry")
    .arg("2")
    .arg("--retry-delay")
    .arg("1")
    .arg("-w")
    .arg("\n__EMBED_DENO_STATUS__%{http_code}");

  if disable_proxy_env {
    command
      .arg("--noproxy")
      .arg("*")
      .arg("--proxy")
      .arg("")
      .env_remove("ALL_PROXY")
      .env_remove("HTTP_PROXY")
      .env_remove("HTTPS_PROXY")
      .env_remove("NO_PROXY")
      .env_remove("all_proxy")
      .env_remove("http_proxy")
      .env_remove("https_proxy")
      .env_remove("no_proxy");
  }

  for (name, value) in headers {
    command.arg("-H").arg(format!("{name}: {value}"));
  }

  if let Some(body) = body {
    command.arg("--data-binary").arg(body);
  }

  command.arg(url).output()
}

#[op2]
#[serde]
fn op_fetch_http(
  #[string] url: String,
  #[string] method: String,
  #[serde] headers: BTreeMap<String, String>,
  #[string] body: Option<String>,
) -> FetchHttpResult {
  if !env_bool("EMBED_DENO_ALLOW_NET", true) {
    return FetchHttpResult {
      status: 0,
      status_text: String::new(),
      body: String::new(),
      error: Some("Network permission denied (set EMBED_DENO_ALLOW_NET=1)".to_string()),
    };
  }

  let primary = run_curl_fetch(&url, &method, &headers, &body, false);
  let output = match primary {
    Ok(output) if output.status.success() => output,
    Ok(primary_output) => {
      let fallback = run_curl_fetch(&url, &method, &headers, &body, true);
      match fallback {
        Ok(fallback_output) if fallback_output.status.success() => fallback_output,
        Ok(fallback_output) => {
          return FetchHttpResult {
            status: 0,
            status_text: String::new(),
            body: String::new(),
            error: Some(format!(
              "Failed to fetch {}\n- with proxy env: {}\n- with direct mode: {}",
              url,
              String::from_utf8_lossy(&primary_output.stderr).trim(),
              String::from_utf8_lossy(&fallback_output.stderr).trim(),
            )),
          };
        }
        Err(error) => {
          return FetchHttpResult {
            status: 0,
            status_text: String::new(),
            body: String::new(),
            error: Some(format!("Failed to execute curl in direct mode: {error}")),
          };
        }
      }
    }
    Err(error) => {
      return FetchHttpResult {
        status: 0,
        status_text: String::new(),
        body: String::new(),
        error: Some(format!("Failed to execute curl: {error}")),
      };
    }
  };

  let output_text = String::from_utf8_lossy(&output.stdout).to_string();
  let marker = "\n__EMBED_DENO_STATUS__";
  let (body, status) = if let Some(index) = output_text.rfind(marker) {
    let body = output_text[.. index].to_string();
    let status_text = output_text[index + marker.len() ..].trim();
    let status = status_text.parse::<u16>().unwrap_or(0);
    (body, status)
  } else {
    (output_text, 0)
  };

  FetchHttpResult {
    status,
    status_text: http_status_text(status).to_string(),
    body,
    error: None,
  }
}

#[op2]
#[serde]
fn op_read_text_file_sync_result(#[string] path: String) -> ReadTextFileSyncResult {
  if !env_bool("EMBED_DENO_ALLOW_READ", true) {
    return ReadTextFileSyncResult {
      ok: false,
      text: None,
      error_kind: Some("PermissionDenied"),
      error_message: Some("Read permission denied (set EMBED_DENO_ALLOW_READ=1)".to_string()),
    };
  }

  match std::fs::read_to_string(path) {
    Ok(text) => ReadTextFileSyncResult {
      ok: true,
      text: Some(text),
      error_kind: None,
      error_message: None,
    },
    Err(error) => {
      let error_kind = match error.kind() {
        std::io::ErrorKind::NotFound => "NotFound",
        std::io::ErrorKind::PermissionDenied => "PermissionDenied",
        _ => "Other",
      };

      ReadTextFileSyncResult {
        ok: false,
        text: None,
        error_kind: Some(error_kind),
        error_message: Some(error.to_string()),
      }
    }
  }
}

deno_core::extension!(
  embed_runtime,
  ops = [
    op_get_random_values,
    op_sleep,
    op_env_snapshot,
    op_read_text_file_sync_result,
    op_sign_jwt_hs256,
    op_fetch_http
  ]
);

#[derive(Clone, Debug)]
struct RuntimeConfig {
  allow_read: bool,
  allow_net: bool,
  cache_dir: PathBuf,
}

impl RuntimeConfig {
  fn from_env() -> Result<Self, AnyError> {
    let allow_read = env_bool("EMBED_DENO_ALLOW_READ", true);
    let allow_net = env_bool("EMBED_DENO_ALLOW_NET", true);

    let cache_dir = if let Ok(path) = env::var("EMBED_DENO_CACHE_DIR") {
      PathBuf::from(path)
    } else {
      default_cache_dir()?
    };

    std::fs::create_dir_all(&cache_dir)?;

    Ok(Self {
      allow_read,
      allow_net,
      cache_dir,
    })
  }
}

#[derive(Clone)]
struct EmbeddedModuleLoader {
  config: RuntimeConfig,
}

impl EmbeddedModuleLoader {
  fn new(config: RuntimeConfig) -> Self {
    Self { config }
  }

  fn resolve_npm_specifier(specifier: &str) -> Result<ModuleSpecifier, ModuleLoaderError> {
    let npm_req = specifier.trim_start_matches("npm:");
    ModuleSpecifier::parse(&format!("https://esm.sh/{npm_req}"))
      .map_err(|error| ModuleLoaderError::generic(error.to_string()))
  }

  fn resolve_jsr_specifier(&self, specifier: &str) -> Result<ModuleSpecifier, ModuleLoaderError> {
    let req_ref = JsrPackageReqReference::from_str(specifier)
      .or_else(|_| {
        specifier
          .strip_prefix("jsr:/")
          .ok_or_else(|| ModuleLoaderError::generic(format!("Invalid JSR specifier: {specifier}")))
          .and_then(|rest| {
            JsrPackageReqReference::from_str(&format!("jsr:{rest}"))
              .map_err(|error| ModuleLoaderError::generic(error.to_string()))
          })
      })
      .map_err(|error| ModuleLoaderError::generic(error.to_string()))?;

    let req = req_ref.req();
    let package_name = req.name.to_string();

    let package_info_url =
      ModuleSpecifier::parse(&format!("https://jsr.io/{package_name}/meta.json"))
        .map_err(|error| ModuleLoaderError::generic(error.to_string()))?;
    let package_info_source = self.fetch_remote_http(&package_info_url)?;
    let package_info: JsrPackageInfo = deno_core::serde_json::from_str(&package_info_source)
      .map_err(|error| {
        ModuleLoaderError::generic(format!(
          "Failed to parse JSR package metadata for {package_name}: {error}"
        ))
      })?;

    let version = package_info
      .versions
      .iter()
      .filter(|(version, info)| !info.yanked && req.version_req.matches(version))
      .map(|(version, _)| version)
      .max()
      .ok_or_else(|| {
        ModuleLoaderError::generic(format!("No matching JSR version for {specifier}"))
      })?;

    let version_info_url = ModuleSpecifier::parse(&format!(
      "https://jsr.io/{package_name}/{version}_meta.json"
    ))
    .map_err(|error| ModuleLoaderError::generic(error.to_string()))?;
    let version_info_source = self.fetch_remote_http(&version_info_url)?;
    let version_info: JsrPackageVersionInfo = deno_core::serde_json::from_str(&version_info_source)
      .map_err(|error| {
        ModuleLoaderError::generic(format!(
          "Failed to parse JSR version metadata for {package_name}@{version}: {error}"
        ))
      })?;

    let export_name = req_ref.export_name();
    let export_path = version_info.export(export_name.as_ref()).ok_or_else(|| {
      ModuleLoaderError::generic(format!(
        "Unable to resolve JSR export '{}' in {specifier}",
        export_name
      ))
    })?;
    let export_path = export_path.strip_prefix("./").unwrap_or(export_path);
    let export_path = export_path.strip_prefix('/').unwrap_or(export_path);

    ModuleSpecifier::parse(&format!(
      "https://jsr.io/{package_name}/{version}/{export_path}"
    ))
    .map_err(|error| ModuleLoaderError::generic(error.to_string()))
  }

  fn curl_fetch(
    &self,
    module_specifier: &ModuleSpecifier,
    disable_proxy_env: bool,
  ) -> Result<Output, ModuleLoaderError> {
    let mut command = Command::new("curl");
    command
      .arg("-fsSL")
      .arg("--connect-timeout")
      .arg("10")
      .arg("--max-time")
      .arg("60")
      .arg("--retry")
      .arg("2")
      .arg("--retry-delay")
      .arg("1");

    if disable_proxy_env {
      command
        .arg("--noproxy")
        .arg("*")
        .arg("--proxy")
        .arg("")
        .env_remove("ALL_PROXY")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("NO_PROXY")
        .env_remove("all_proxy")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("no_proxy");
    }

    command.arg(module_specifier.as_str());

    command
      .output()
      .map_err(|error| ModuleLoaderError::generic(error.to_string()))
  }

  fn patch_remote_source(module_specifier: &ModuleSpecifier, source: String) -> String {
    let spec = module_specifier.as_str();
    if !spec.contains("esm.sh/")
      || !spec.contains("stream-chat@")
      || !spec.contains("stream-chat.mjs")
    {
      return source;
    }

    source
      .replace(
        "this.node&&!this.options.httpsAgent&&(this.options.httpsAgent=new \
         Os.default.Agent({keepAlive:!0,keepAliveMsecs:3e3}))",
        "this.node&&!this.options.httpsAgent&&(this.options.httpsAgent={})",
      )
      .replace(
        "if(he.default==null||he.default.sign==null)throw Error(\"Unable to find jwt crypto, if \
         you are getting this error is probably because you are trying to generate tokens on \
         browser or React Native (or other environment where crypto functions are not available). \
         Please Note: token should only be generated server-side.\")",
        "if(he.default==null||he.default.sign==null){}",
      )
      .replace(
        "return n.iat&&(a.noTimestamp=!1),he.default.sign(n,t,a)",
        "return n.iat&&(a.noTimestamp=!1),he.default&&he.default.sign?he.default.sign(n,t,a):Deno.\
         core.ops.op_sign_jwt_hs256(t,n,a.noTimestamp===!0)",
      )
      .replace(
        "return he.default.sign(s,t,i)",
        "return he.default&&he.default.sign?he.default.sign(s,t,i):Deno.core.ops.\
         op_sign_jwt_hs256(t,s,i.noTimestamp===!0)",
      )
  }

  fn read_local_file(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<String, ModuleLoaderError> {
    if !self.config.allow_read {
      return Err(ModuleLoaderError::generic(
        "Read permission denied (set EMBED_DENO_ALLOW_READ=1)",
      ));
    }

    let path = module_specifier
      .to_file_path()
      .map_err(|_| ModuleLoaderError::generic(format!("Not a file URL: {module_specifier}")))?;

    std::fs::read_to_string(path).map_err(|error| ModuleLoaderError::generic(error.to_string()))
  }

  fn fetch_remote_http(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<String, ModuleLoaderError> {
    if !self.config.allow_net {
      return Err(ModuleLoaderError::generic(
        "Network permission denied (set EMBED_DENO_ALLOW_NET=1)",
      ));
    }

    if let Some(cached) = self.read_remote_cache(module_specifier)? {
      return Ok(cached);
    }

    let primary_output = self.curl_fetch(module_specifier, false)?;
    let output = if primary_output.status.success() {
      primary_output
    } else {
      let fallback_output = self.curl_fetch(module_specifier, true)?;
      if fallback_output.status.success() {
        fallback_output
      } else {
        return Err(ModuleLoaderError::generic(format!(
          "Failed to fetch {}\n- with proxy env: {}\n- with direct mode: {}",
          module_specifier,
          String::from_utf8_lossy(&primary_output.stderr).trim(),
          String::from_utf8_lossy(&fallback_output.stderr).trim(),
        )));
      }
    };

    if !output.status.success() {
      return Err(ModuleLoaderError::generic(format!(
        "Failed to fetch {}: {}",
        module_specifier,
        String::from_utf8_lossy(&output.stderr)
      )));
    }

    let text = String::from_utf8(output.stdout)
      .map_err(|error| ModuleLoaderError::generic(error.to_string()))?;

    self.write_remote_cache(module_specifier, &text)?;
    Ok(text)
  }

  fn cache_file_path(&self, module_specifier: &ModuleSpecifier) -> PathBuf {
    let hash = format!(
      "{:016x}",
      twox_hash::XxHash64::oneshot(0, module_specifier.as_str().as_bytes())
    );
    self.config.cache_dir.join(format!("{hash}.mjs"))
  }

  fn read_remote_cache(
    &self,
    module_specifier: &ModuleSpecifier,
  ) -> Result<Option<String>, ModuleLoaderError> {
    let path = self.cache_file_path(module_specifier);
    if !path.exists() {
      return Ok(None);
    }

    let text = std::fs::read_to_string(path)
      .map_err(|error| ModuleLoaderError::generic(error.to_string()))?;
    Ok(Some(text))
  }

  fn write_remote_cache(
    &self,
    module_specifier: &ModuleSpecifier,
    content: &str,
  ) -> Result<(), ModuleLoaderError> {
    let path = self.cache_file_path(module_specifier);
    std::fs::write(path, content).map_err(|error| ModuleLoaderError::generic(error.to_string()))
  }

  fn maybe_transpile(
    module_specifier: &ModuleSpecifier,
    source: String,
  ) -> Result<(ModuleType, String), ModuleLoaderError> {
    let media_type = MediaType::from_specifier(module_specifier);
    let needs_transpile = matches!(
      media_type,
      MediaType::TypeScript | MediaType::Mts | MediaType::Cts | MediaType::Tsx | MediaType::Jsx
    );

    if !needs_transpile {
      let module_type = if media_type == MediaType::Json {
        ModuleType::Json
      } else {
        ModuleType::JavaScript
      };
      return Ok((module_type, source));
    }

    let parsed = deno_ast::parse_module(ParseParams {
      specifier: module_specifier.clone(),
      text: source.into(),
      media_type,
      capture_tokens: false,
      maybe_syntax: None,
      scope_analysis: false,
    })
    .map_err(|error| ModuleLoaderError::generic(error.to_string()))?;

    let transpile_options = deno_ast::TranspileOptions::default();
    let transpile_module_options = deno_ast::TranspileModuleOptions::default();
    let emit_options = EmitOptions {
      source_map: SourceMapOption::None,
      ..Default::default()
    };

    let transpiled = parsed
      .transpile(&transpile_options, &transpile_module_options, &emit_options)
      .map_err(|error| ModuleLoaderError::generic(error.to_string()))?;

    Ok((ModuleType::JavaScript, transpiled.into_source().text))
  }
}

impl ModuleLoader for EmbeddedModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, ModuleLoaderError> {
    if specifier.starts_with("npm:") {
      return Self::resolve_npm_specifier(specifier);
    }

    if specifier.starts_with("jsr:") {
      return self.resolve_jsr_specifier(specifier);
    }

    if kind == ResolutionKind::MainModule {
      return if specifier.contains("://") {
        ModuleSpecifier::parse(specifier)
          .map_err(|error| ModuleLoaderError::generic(error.to_string()))
      } else {
        let absolute = std::fs::canonicalize(Path::new(specifier))
          .map_err(|error| ModuleLoaderError::generic(error.to_string()))?;
        ModuleSpecifier::from_file_path(absolute)
          .map_err(|_| ModuleLoaderError::generic(format!("Invalid script path: {specifier}")))
      };
    }

    resolve_import(specifier, referrer)
      .map_err(|error| ModuleLoaderError::generic(error.to_string()))
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: deno_core::ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();
    let this = self.clone();

    ModuleLoadResponse::Async(
      async move {
        let source = match module_specifier.scheme() {
          "file" => this.read_local_file(&module_specifier)?,
          "http" | "https" => this.fetch_remote_http(&module_specifier)?,
          scheme => {
            return Err(ModuleLoaderError::generic(format!(
              "Unsupported module scheme '{scheme}' for {module_specifier}"
            )));
          }
        };

        let source = Self::patch_remote_source(&module_specifier, source);
        let (module_type, code) = Self::maybe_transpile(&module_specifier, source)?;

        Ok(ModuleSource::new(
          module_type,
          ModuleSourceCode::String(deno_core::ModuleCodeString::from(code)),
          &module_specifier,
          None,
        ))
      }
      .boxed(),
    )
  }
}

fn env_bool(name: &str, default: bool) -> bool {
  match env::var(name) {
    Ok(value) => matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
    Err(_) => default,
  }
}

fn default_cache_dir() -> Result<PathBuf, AnyError> {
  let home = env::var("HOME").map_err(|_| AnyError::msg("$HOME is not set"))?;
  Ok(
    Path::new(&home)
      .join(".embed_deno_cache")
      .join("remote_modules"),
  )
}

fn main() {
  if let Err(error) = run() {
    eprintln!("{error:#}");
    std::process::exit(1);
  }
}

fn run() -> Result<(), AnyError> {
  let mut args = env::args().skip(1);
  let Some(main_module_arg) = args.next() else {
    return Err(AnyError::msg(
      "Usage: embed_deno <entry.ts|entry.js|npm:pkg>",
    ));
  };

  let config = RuntimeConfig::from_env()?;
  let loader = Rc::new(EmbeddedModuleLoader::new(config));
  let mut runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(loader.clone()),
    extensions: vec![embed_runtime::init()],
    ..Default::default()
  });

  runtime.execute_script(
    "bootstrap://runtime_primitives",
    r#"
if (!globalThis.crypto) {
  globalThis.crypto = {
    getRandomValues(typedArray) {
      if (!typedArray || typeof typedArray.length !== "number") {
        throw new TypeError("Expected an integer typed array");
      }
      Deno.core.ops.op_get_random_values(typedArray);
      return typedArray;
    },
    randomUUID() {
      const bytes = this.getRandomValues(new Uint8Array(16));
      bytes[6] = (bytes[6] & 0x0f) | 0x40;
      bytes[8] = (bytes[8] & 0x3f) | 0x80;
      const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0"));
      return (
        hex.slice(0, 4).join("") + "-" +
        hex.slice(4, 6).join("") + "-" +
        hex.slice(6, 8).join("") + "-" +
        hex.slice(8, 10).join("") + "-" +
        hex.slice(10, 16).join("")
      );
    },
  };
}

if (!globalThis.setTimeout) {
  let nextTimerId = 1;
  const timers = new Map();

  function normalizeDelay(delay) {
    if (typeof delay !== "number" || !Number.isFinite(delay) || delay < 0) {
      return 0;
    }
    return Math.floor(delay);
  }

  function ensureCallback(callback) {
    if (typeof callback !== "function") {
      throw new TypeError("Timer callback must be a function");
    }
  }

  globalThis.setTimeout = (callback, delay = 0, ...args) => {
    ensureCallback(callback);
    const timerId = nextTimerId++;
    const timerState = { cancelled: false, repeat: false };
    timers.set(timerId, timerState);

    void (async () => {
      await Deno.core.ops.op_sleep(normalizeDelay(delay));
      if (!timerState.cancelled) {
        try {
          callback(...args);
        } finally {
          timers.delete(timerId);
        }
      }
    })();

    return timerId;
  };

  globalThis.clearTimeout = (timerId) => {
    const timerState = timers.get(timerId);
    if (timerState) {
      timerState.cancelled = true;
      timers.delete(timerId);
    }
  };

  globalThis.setInterval = (callback, delay = 0, ...args) => {
    ensureCallback(callback);
    const timerId = nextTimerId++;
    const timerState = { cancelled: false, repeat: true };
    timers.set(timerId, timerState);

    void (async () => {
      const normalizedDelay = normalizeDelay(delay);
      while (!timerState.cancelled) {
        await Deno.core.ops.op_sleep(normalizedDelay);
        if (timerState.cancelled) {
          break;
        }
        callback(...args);
      }
      timers.delete(timerId);
    })();

    return timerId;
  };

  globalThis.clearInterval = globalThis.clearTimeout;
}

if (!globalThis.btoa || !globalThis.atob) {
  const BASE64_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

  if (!globalThis.btoa) {
    globalThis.btoa = (input) => {
      const str = String(input);
      let output = "";

      for (let index = 0; index < str.length; index += 3) {
        const byte1 = str.charCodeAt(index);
        const hasByte2 = index + 1 < str.length;
        const hasByte3 = index + 2 < str.length;
        const byte2 = hasByte2 ? str.charCodeAt(index + 1) : 0;
        const byte3 = hasByte3 ? str.charCodeAt(index + 2) : 0;

        if (byte1 > 0xff || byte2 > 0xff || byte3 > 0xff) {
          throw new TypeError("The string to be encoded contains characters outside Latin1 range.");
        }

        const chunk = (byte1 << 16) | (byte2 << 8) | byte3;
        const char1 = BASE64_CHARS[(chunk >> 18) & 0x3f];
        const char2 = BASE64_CHARS[(chunk >> 12) & 0x3f];
        const char3 = hasByte2 ? BASE64_CHARS[(chunk >> 6) & 0x3f] : "=";
        const char4 = hasByte3 ? BASE64_CHARS[chunk & 0x3f] : "=";
        output += char1 + char2 + char3 + char4;
      }

      return output;
    };
  }

  if (!globalThis.atob) {
    globalThis.atob = (input) => {
      let str = String(input).replace(/[\t\n\f\r ]+/g, "");
      str = str.replace(/-/g, "+").replace(/_/g, "/");

      if (str.length % 4 === 1) {
        throw new TypeError("Invalid base64 input");
      }

      while (str.length % 4 !== 0) {
        str += "=";
      }

      let output = "";

      for (let index = 0; index < str.length; index += 4) {
        const c1 = str[index];
        const c2 = str[index + 1];
        const c3 = str[index + 2];
        const c4 = str[index + 3];

        const v1 = BASE64_CHARS.indexOf(c1);
        const v2 = BASE64_CHARS.indexOf(c2);
        const v3 = c3 === "=" ? -1 : BASE64_CHARS.indexOf(c3);
        const v4 = c4 === "=" ? -1 : BASE64_CHARS.indexOf(c4);

        if (v1 < 0 || v2 < 0 || (c3 !== "=" && v3 < 0) || (c4 !== "=" && v4 < 0)) {
          throw new TypeError("Invalid base64 input");
        }

        const chunk = (v1 << 18) | (v2 << 12) | ((v3 < 0 ? 0 : v3) << 6) | (v4 < 0 ? 0 : v4);
        output += String.fromCharCode((chunk >> 16) & 0xff);
        if (c3 !== "=") {
          output += String.fromCharCode((chunk >> 8) & 0xff);
        }
        if (c4 !== "=") {
          output += String.fromCharCode(chunk & 0xff);
        }
      }

      return output;
    };
  }
}

if (!globalThis.URLSearchParams) {
  const encode = (value) =>
    encodeURIComponent(String(value)).replace(/%20/g, "+");
  const decode = (value) =>
    decodeURIComponent(String(value).replace(/\+/g, "%20"));

  class URLSearchParams {
    constructor(init = "") {
      this._entries = [];

      if (typeof init === "string") {
        const source = init.startsWith("?") ? init.slice(1) : init;
        if (!source) {
          return;
        }
        for (const segment of source.split("&")) {
          if (!segment) {
            continue;
          }
          const index = segment.indexOf("=");
          if (index === -1) {
            this.append(decode(segment), "");
          } else {
            const key = segment.slice(0, index);
            const value = segment.slice(index + 1);
            this.append(decode(key), decode(value));
          }
        }
        return;
      }

      if (Array.isArray(init) || (init && typeof init[Symbol.iterator] === "function")) {
        for (const pair of init) {
          if (!pair || pair.length < 2) {
            continue;
          }
          this.append(pair[0], pair[1]);
        }
        return;
      }

      if (init && typeof init === "object") {
        for (const key of Object.keys(init)) {
          this.append(key, init[key]);
        }
      }
    }

    append(name, value) {
      this._entries.push([String(name), String(value)]);
    }

    delete(name, value) {
      const key = String(name);
      if (arguments.length >= 2) {
        const expected = String(value);
        this._entries = this._entries.filter(
          ([entryName, entryValue]) => !(entryName === key && entryValue === expected),
        );
      } else {
        this._entries = this._entries.filter(([entryName]) => entryName !== key);
      }
    }

    get(name) {
      const key = String(name);
      const found = this._entries.find(([entryName]) => entryName === key);
      return found ? found[1] : null;
    }

    getAll(name) {
      const key = String(name);
      return this._entries
        .filter(([entryName]) => entryName === key)
        .map(([, value]) => value);
    }

    has(name, value) {
      const key = String(name);
      if (arguments.length >= 2) {
        const expected = String(value);
        return this._entries.some(
          ([entryName, entryValue]) => entryName === key && entryValue === expected,
        );
      }
      return this._entries.some(([entryName]) => entryName === key);
    }

    set(name, value) {
      const key = String(name);
      const nextValue = String(value);
      let replaced = false;
      const nextEntries = [];
      for (const [entryName, entryValue] of this._entries) {
        if (entryName === key) {
          if (!replaced) {
            nextEntries.push([key, nextValue]);
            replaced = true;
          }
        } else {
          nextEntries.push([entryName, entryValue]);
        }
      }
      if (!replaced) {
        nextEntries.push([key, nextValue]);
      }
      this._entries = nextEntries;
    }

    sort() {
      this._entries.sort(([left], [right]) => left.localeCompare(right));
    }

    forEach(callback, thisArg = undefined) {
      for (const [name, value] of this._entries) {
        callback.call(thisArg, value, name, this);
      }
    }

    keys() {
      return this._entries.map(([name]) => name)[Symbol.iterator]();
    }

    values() {
      return this._entries.map(([, value]) => value)[Symbol.iterator]();
    }

    entries() {
      return this._entries.map(([name, value]) => [name, value])[Symbol.iterator]();
    }

    [Symbol.iterator]() {
      return this.entries();
    }

    toString() {
      return this._entries
        .map(([name, value]) => `${encode(name)}=${encode(value)}`)
        .join("&");
    }

    get [Symbol.toStringTag]() {
      return "URLSearchParams";
    }
  }

  globalThis.URLSearchParams = URLSearchParams;
}

if (!globalThis.Blob) {
  function partToUint8Array(part) {
    if (part instanceof Uint8Array) {
      return new Uint8Array(part);
    }
    if (typeof ArrayBuffer !== "undefined" && part instanceof ArrayBuffer) {
      return new Uint8Array(part.slice(0));
    }
    if (typeof ArrayBuffer !== "undefined" && ArrayBuffer.isView(part)) {
      return new Uint8Array(
        part.buffer.slice(part.byteOffset, part.byteOffset + part.byteLength),
      );
    }
    if (part instanceof Blob) {
      return part._bytes();
    }
    if (typeof part === "string") {
      return new TextEncoder().encode(part);
    }
    return new TextEncoder().encode(String(part));
  }

  class Blob {
    constructor(blobParts = [], options = {}) {
      this.type = options?.type ? String(options.type).toLowerCase() : "";
      this._chunks = [];
      this.size = 0;

      for (const part of blobParts) {
        const chunk = partToUint8Array(part);
        this._chunks.push(chunk);
        this.size += chunk.byteLength;
      }
    }

    _bytes() {
      const result = new Uint8Array(this.size);
      let offset = 0;
      for (const chunk of this._chunks) {
        result.set(chunk, offset);
        offset += chunk.byteLength;
      }
      return result;
    }

    async arrayBuffer() {
      const bytes = this._bytes();
      return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    }

    async text() {
      return new TextDecoder().decode(this._bytes());
    }

    slice(start = 0, end = this.size, contentType = "") {
      const bytes = this._bytes();
      const normalizedStart = Math.max(0, Math.min(this.size, Number(start) || 0));
      const normalizedEnd = Math.max(normalizedStart, Math.min(this.size, Number(end) || 0));
      return new Blob([bytes.slice(normalizedStart, normalizedEnd)], {
        type: contentType,
      });
    }

    stream() {
      const bytes = this._bytes();
      return new ReadableStream({
        start(controller) {
          controller.enqueue(bytes);
          controller.close();
        },
      });
    }

    get [Symbol.toStringTag]() {
      return "Blob";
    }
  }

  globalThis.Blob = Blob;
}

if (!globalThis.File) {
  class File extends Blob {
    constructor(fileBits, fileName, options = {}) {
      super(fileBits, options);
      this.name = String(fileName);
      this.lastModified = Number(options?.lastModified) || Date.now();
    }

    get [Symbol.toStringTag]() {
      return "File";
    }
  }

  globalThis.File = File;
}

if (!globalThis.Deno.env) {
  const envStore = new Map(Object.entries(Deno.core.ops.op_env_snapshot()));

  function ensureEnvKey(key) {
    if (typeof key !== "string") {
      throw new TypeError("Environment variable key must be a string");
    }
    if (key.length === 0) {
      throw new TypeError("Environment variable key must not be empty");
    }
  }

  globalThis.Deno.env = {
    get(key) {
      ensureEnvKey(key);
      return envStore.get(key);
    },
    set(key, value) {
      ensureEnvKey(key);
      envStore.set(key, String(value));
    },
    has(key) {
      ensureEnvKey(key);
      return envStore.has(key);
    },
    delete(key) {
      ensureEnvKey(key);
      envStore.delete(key);
    },
    toObject() {
      return Object.fromEntries(envStore);
    },
  };
}

if (!globalThis.Deno.readTextFileSync) {
  if (!globalThis.Deno.errors) {
    class NotFound extends Error {
      constructor(message = "Not found") {
        super(message);
        this.name = "NotFound";
      }
    }

    class PermissionDenied extends Error {
      constructor(message = "Permission denied") {
        super(message);
        this.name = "PermissionDenied";
      }
    }

    globalThis.Deno.errors = {
      NotFound,
      PermissionDenied,
    };
  }

  function resolveReadPath(path) {
    if (typeof URL !== "undefined" && path instanceof URL) {
      if (path.protocol !== "file:") {
        throw new TypeError("Only file: URLs are supported");
      }
      return path.pathname;
    }
    return String(path);
  }

  globalThis.Deno.readTextFileSync = (path) => {
    const result = Deno.core.ops.op_read_text_file_sync_result(resolveReadPath(path));
    if (result.ok) {
      return result.text;
    }

    if (result.errorKind === "NotFound") {
      throw new Deno.errors.NotFound(result.errorMessage || "Not found");
    }
    if (result.errorKind === "PermissionDenied") {
      throw new Deno.errors.PermissionDenied(
        result.errorMessage || "Permission denied",
      );
    }

    throw new Error(result.errorMessage || "Failed to read file");
  };
}

if (!globalThis.fetch) {
  class Headers {
    constructor(init = {}) {
      this._map = new Map();

      if (init instanceof Headers) {
        for (const [name, value] of init.entries()) {
          this.append(name, value);
        }
      } else if (Array.isArray(init)) {
        for (const pair of init) {
          if (!pair || pair.length < 2) {
            continue;
          }
          this.append(pair[0], pair[1]);
        }
      } else if (init && typeof init === "object") {
        for (const key of Object.keys(init)) {
          this.append(key, init[key]);
        }
      }
    }

    _normalizeName(name) {
      return String(name).toLowerCase();
    }

    append(name, value) {
      const key = this._normalizeName(name);
      const existing = this._map.get(key);
      const nextValue = String(value);
      this._map.set(key, existing ? `${existing}, ${nextValue}` : nextValue);
    }

    set(name, value) {
      this._map.set(this._normalizeName(name), String(value));
    }

    get(name) {
      const value = this._map.get(this._normalizeName(name));
      return value === undefined ? null : value;
    }

    has(name) {
      return this._map.has(this._normalizeName(name));
    }

    delete(name) {
      this._map.delete(this._normalizeName(name));
    }

    entries() {
      return this._map.entries();
    }

    keys() {
      return this._map.keys();
    }

    values() {
      return this._map.values();
    }

    [Symbol.iterator]() {
      return this.entries();
    }

    forEach(callback, thisArg = undefined) {
      for (const [name, value] of this._map) {
        callback.call(thisArg, value, name, this);
      }
    }
  }

  class Response {
    constructor(body = "", init = {}) {
      this._body = String(body ?? "");
      this.status = Number(init.status ?? 200);
      this.statusText = String(init.statusText ?? "");
      this.headers = init.headers instanceof Headers ? init.headers : new Headers(init.headers);
      this.ok = this.status >= 200 && this.status < 300;
      this.redirected = false;
      this.type = "basic";
      this.url = init.url ?? "";
      this.bodyUsed = false;
    }

    async text() {
      this.bodyUsed = true;
      return this._body;
    }

    async json() {
      this.bodyUsed = true;
      return JSON.parse(this._body);
    }
  }

  globalThis.Headers = globalThis.Headers || Headers;
  globalThis.Response = globalThis.Response || Response;

  globalThis.fetch = async (input, init = {}) => {
    const url = typeof input === "string" ? input : String(input?.url ?? input);
    const method = String(init.method ?? "GET").toUpperCase();

    const headers = new Headers(init.headers ?? {});
    const headerObject = Object.fromEntries(headers.entries());

    let body = null;
    if (init.body != null) {
      if (typeof init.body === "string") {
        body = init.body;
      } else if (init.body instanceof Uint8Array) {
        body = new TextDecoder().decode(init.body);
      } else {
        body = String(init.body);
      }
    }

    const result = Deno.core.ops.op_fetch_http(url, method, headerObject, body);
    if (result.error) {
      throw new Error(result.error);
    }

    return new Response(result.body, {
      status: result.status,
      statusText: result.statusText,
      headers,
      url,
    });
  };
}
"#,
  )?;

  let main_specifier = loader.resolve(&main_module_arg, ".", ResolutionKind::MainModule)?;

  let tokio_runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .map_err(|error| AnyError::msg(error.to_string()))?;

  tokio_runtime.block_on(async move {
    let module_id = runtime.load_main_es_module(&main_specifier).await?;
    let result_receiver = runtime.mod_evaluate(module_id);
    runtime.run_event_loop(Default::default()).await?;
    result_receiver.await?;
    Ok::<(), AnyError>(())
  })?;

  Ok(())
}
