use std::{
  borrow::Cow,
  cell::RefCell,
  collections::{BTreeSet, HashMap, HashSet},
  fmt::Write as _,
  path::{Path, PathBuf},
  rc::Rc,
  sync::{Arc, Mutex},
  time::Duration,
};

use base64::Engine;
use deno_ast::{MediaType, ParseParams, SourceMapOption};
use deno_cache_dir::npm::NpmCacheDir;
use deno_core::{
  ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader, ModuleSource,
  ModuleSourceCode, ModuleType, ResolutionKind, error::ModuleLoaderError, resolve_import, url::Url,
};
use deno_error::JsErrorBox;
use deno_graph::packages::{JsrPackageInfo, JsrPackageVersionInfo, JsrVersionResolver};
use deno_npm::{
  npm_rc::ResolvedNpmRc,
  registry::{NpmPackageVersionDistInfo, NpmRegistryApi},
  resolution::NpmVersionResolver,
};
use deno_npm_cache::{
  DownloadError, NpmCache, NpmCacheHttpClient, NpmCacheHttpClientBytesResponse,
  NpmCacheHttpClientResponse, NpmCacheSetting, RegistryInfoProvider, TarballCache,
};
use deno_semver::{
  jsr::JsrPackageReqReference,
  npm::NpmPackageReqReference,
  package::{PackageNv, PackageReq},
};
use once_cell::sync::Lazy;
use regex::Regex;
use sha2::{Digest, Sha256};
use sys_traits::impls::RealSys;

type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

fn trace_module_loader_enabled() -> bool {
  std::env::var_os("LIBMAINWORKER_TRACE_MODULE_LOADER").is_some()
}

fn is_truthy_flag(value: &str) -> bool {
  matches!(
    value.trim().to_ascii_lowercase().as_str(),
    "1" | "true" | "yes" | "on"
  )
}

fn first_non_empty_env_var(keys: &[&str]) -> Option<String> {
  for key in keys {
    if let Ok(value) = std::env::var(key) {
      let trimmed = value.trim();
      if !trimmed.is_empty() {
        return Some(trimmed.to_string());
      }
    }
  }
  None
}

fn configured_ureq_agent() -> ureq::Agent {
  let mut builder = ureq::Agent::config_builder();
  if let Some(proxy_value) = first_non_empty_env_var(&[
    "https_proxy",
    "HTTPS_PROXY",
    "http_proxy",
    "HTTP_PROXY",
    "all_proxy",
    "ALL_PROXY",
  ]) {
    if let Ok(proxy_url) = ureq::Proxy::new(&proxy_value) {
      builder = builder.proxy(Some(proxy_url));
    }
  }
  ureq::Agent::new_with_config(builder.build())
}

fn deno_dir_root() -> Option<PathBuf> {
  if let Some(deno_dir) = std::env::var_os("DENO_DIR") {
    return Some(PathBuf::from(deno_dir));
  }
  std::env::current_dir()
    .ok()
    .map(|cwd| cwd.join(".libmainworker_deno_dir"))
}

fn sha256_hex(input: &str) -> String {
  let digest = Sha256::digest(input.as_bytes());
  let mut out = String::with_capacity(digest.len() * 2);
  for byte in digest {
    let _ = write!(&mut out, "{byte:02x}");
  }
  out
}

fn cache_file_path(namespace: &str, url: &str, ext: &str) -> Option<PathBuf> {
  let root = deno_dir_root()?;
  let hash = sha256_hex(url);
  Some(root.join(namespace).join(format!("{hash}.{ext}")))
}

fn read_cached_text(namespace: &str, url: &str) -> Option<String> {
  let path = cache_file_path(namespace, url, "txt")?;
  let bytes = std::fs::read(path).ok()?;
  String::from_utf8(bytes).ok()
}

fn write_cached_text(namespace: &str, url: &str, text: &str) {
  let Some(path) = cache_file_path(namespace, url, "txt") else {
    return;
  };
  if let Some(parent) = path.parent() {
    if std::fs::create_dir_all(parent).is_err() {
      return;
    }
  }
  let _ = std::fs::write(path, text.as_bytes());
}

fn read_cached_bytes(namespace: &str, url: &str) -> Option<Vec<u8>> {
  let path = cache_file_path(namespace, url, "json")?;
  std::fs::read(path).ok()
}

fn write_cached_bytes(namespace: &str, url: &str, bytes: &[u8]) {
  let Some(path) = cache_file_path(namespace, url, "json") else {
    return;
  };
  if let Some(parent) = path.parent() {
    if std::fs::create_dir_all(parent).is_err() {
      return;
    }
  }
  let _ = std::fs::write(path, bytes);
}

fn to_double_quote_string(text: &str) -> String {
  serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string())
}

static JS_RESERVED_WORDS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
  HashSet::from([
    "abstract",
    "arguments",
    "async",
    "await",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "double",
    "else",
    "enum",
    "eval",
    "export",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "function",
    "get",
    "goto",
    "if",
    "implements",
    "import",
    "in",
    "instanceof",
    "int",
    "interface",
    "let",
    "long",
    "mod",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "set",
    "short",
    "static",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "volatile",
    "while",
    "with",
    "yield",
  ])
});

static DISABLED_COMMONJS_STUB_RE: Lazy<Regex> = Lazy::new(|| {
  Regex::new(
    r#"(?ms)var (?P<var_name>require_[A-Za-z0-9_]+)\s*=\s*__commonJS\(\{\s*"\(disabled\):(?P<label>[^"]+)"\(\)\s*\{\s*\}\s*\}\);\s*"#,
  )
  .expect("valid disabled commonjs stub regex")
});

static FORM_DATA_IMPORT_RE: Lazy<Regex> = Lazy::new(|| {
  Regex::new(r#"(?m)^import\s+([A-Za-z_$][A-Za-z0-9_$]*)\s+from\s+["']form-data["'];\s*$"#)
    .expect("valid form-data import regex")
});

fn create_default_npmrc() -> Arc<ResolvedNpmRc> {
  Arc::new(ResolvedNpmRc {
    default_config: deno_npm::npm_rc::RegistryConfigWithUrl {
      registry_url: Url::parse("https://registry.npmjs.org").unwrap(),
      config: Default::default(),
    },
    scopes: Default::default(),
    registry_configs: Default::default(),
  })
}

#[derive(Debug, Default)]
struct DirectNpmCacheHttpClient;

#[async_trait::async_trait(?Send)]
impl NpmCacheHttpClient for DirectNpmCacheHttpClient {
  async fn download_with_retries_on_any_tokio_runtime(
    &self,
    url: deno_core::url::Url,
    maybe_auth: Option<String>,
    maybe_etag: Option<String>,
  ) -> Result<NpmCacheHttpClientResponse, DownloadError> {
    let mut retries = 0_u8;

    loop {
      let agent = configured_ureq_agent();
      let mut request = agent.get(url.as_str()).header(
        "user-agent",
        "libmainworker_duplex_stream_example/npm-cache",
      );

      if let Some(auth) = maybe_auth.as_ref() {
        request = request.header("authorization", auth);
      }
      if let Some(etag) = maybe_etag.as_ref() {
        request = request.header("if-none-match", etag);
      }

      let response = request.call();

      match response {
        Ok(response) => {
          let status = response.status().as_u16();
          if status == 304 {
            return Ok(NpmCacheHttpClientResponse::NotModified);
          }
          if status == 404 {
            return Ok(NpmCacheHttpClientResponse::NotFound);
          }

          let etag = response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.to_string());

          let bytes = response
            .into_body()
            .with_config()
            .limit(1024 * 1024 * 256)
            .read_to_vec()
            .map_err(|err| DownloadError {
              status_code: Some(status),
              error: JsErrorBox::generic(format!(
                "failed reading npm response body {}: {err}",
                url
              )),
            })?;

          return Ok(NpmCacheHttpClientResponse::Bytes(
            NpmCacheHttpClientBytesResponse { bytes, etag },
          ));
        }
        Err(ureq::Error::StatusCode(304)) => {
          return Ok(NpmCacheHttpClientResponse::NotModified);
        }
        Err(ureq::Error::StatusCode(404)) => {
          return Ok(NpmCacheHttpClientResponse::NotFound);
        }
        Err(ureq::Error::StatusCode(status_code)) => {
          return Err(DownloadError {
            status_code: Some(status_code),
            error: JsErrorBox::generic(format!("npm http status {} for {}", status_code, url)),
          });
        }
        Err(err) if retries < 1 => {
          retries += 1;
          std::thread::sleep(Duration::from_millis(50));
          eprintln!(
            "retrying npm cache download for {} after error: {}",
            url, err
          );
        }
        Err(err) => {
          return Err(DownloadError {
            status_code: None,
            error: JsErrorBox::generic(format!("npm download failed for {}: {}", url, err)),
          });
        }
      }
    }
  }
}

#[derive(Debug)]
struct NpmPackageResolver {
  npm_cache: Arc<NpmCache<RealSys>>,
  registry_info_provider: Arc<RegistryInfoProvider<DirectNpmCacheHttpClient, RealSys>>,
  tarball_cache: Arc<TarballCache<DirectNpmCacheHttpClient, RealSys>>,
  cache_root: PathBuf,
  version_resolver: NpmVersionResolver,
  resolved_specifier_cache: Mutex<HashMap<String, deno_core::ModuleSpecifier>>,
  package_nv_by_folder: Mutex<HashMap<PathBuf, PackageNv>>,
  dependency_nv_by_parent: Mutex<HashMap<PackageNv, HashMap<String, PackageNv>>>,
  package_type_by_dir: Mutex<HashMap<PathBuf, Option<String>>>,
  use_browser_mappings: bool,
}

impl NpmPackageResolver {
  fn new() -> Result<Self, ModuleLoaderError> {
    let sys = RealSys::default();
    let npmrc = create_default_npmrc();

    let cache_root = if let Some(deno_dir) = std::env::var_os("DENO_DIR") {
      PathBuf::from(deno_dir).join("npm")
    } else {
      std::env::current_dir()
        .map_err(JsErrorBox::from_err)?
        .join(".libmainworker_deno_dir")
        .join("npm")
    };

    let known_registry_urls = vec![npmrc.default_config.registry_url.clone()];
    let npm_cache_dir = Arc::new(NpmCacheDir::new(
      &sys,
      cache_root.clone(),
      known_registry_urls,
    ));
    let npm_cache = Arc::new(NpmCache::new(
      npm_cache_dir,
      sys.clone(),
      NpmCacheSetting::Use,
      npmrc.clone(),
    ));

    let http_client = Arc::new(DirectNpmCacheHttpClient);
    let registry_info_provider = Arc::new(RegistryInfoProvider::new(
      npm_cache.clone(),
      http_client.clone(),
      npmrc.clone(),
    ));
    let tarball_cache = Arc::new(TarballCache::new(
      npm_cache.clone(),
      http_client,
      sys,
      npmrc,
      None,
    ));

    let use_browser_mappings = std::env::var("LIBMAINWORKER_NPM_BROWSER_MAP")
      .map(|value| is_truthy_flag(&value))
      .unwrap_or(true);

    Ok(Self {
      npm_cache,
      registry_info_provider,
      tarball_cache,
      cache_root,
      version_resolver: NpmVersionResolver::default(),
      resolved_specifier_cache: Mutex::new(HashMap::new()),
      package_nv_by_folder: Mutex::new(HashMap::new()),
      dependency_nv_by_parent: Mutex::new(HashMap::new()),
      package_type_by_dir: Mutex::new(HashMap::new()),
      use_browser_mappings,
    })
  }

  fn parse_npm_specifier(specifier: &str) -> Result<deno_core::ModuleSpecifier, ModuleLoaderError> {
    deno_core::ModuleSpecifier::parse(specifier).map_err(JsErrorBox::from_err)
  }

  fn parse_bare_dependency_specifier(specifier: &str) -> Option<(String, Option<String>)> {
    if specifier.starts_with('.') || specifier.starts_with('/') || specifier.contains(':') {
      return None;
    }

    if specifier.starts_with('@') {
      let mut parts = specifier.splitn(3, '/');
      let scope = parts.next()?;
      let name = parts.next()?;
      let dep_name = format!("{scope}/{name}");
      let sub_path = parts.next().map(|value| value.to_string());
      Some((dep_name, sub_path))
    } else {
      let mut parts = specifier.splitn(2, '/');
      let dep_name = parts.next()?.to_string();
      let sub_path = parts.next().map(|value| value.to_string());
      Some((dep_name, sub_path))
    }
  }

  fn is_in_npm_cache_path(&self, path: &Path) -> bool {
    path.starts_with(&self.cache_root)
  }

  fn package_type_for_module_path(&self, module_path: &Path) -> Option<String> {
    let mut current_dir = module_path.parent();
    while let Some(dir) = current_dir {
      if !dir.starts_with(&self.cache_root) {
        break;
      }

      if let Ok(cache) = self.package_type_by_dir.lock() {
        if let Some(found) = cache.get(dir) {
          return found.clone();
        }
      }

      let package_type = std::fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|json| {
          json
            .get("type")
            .and_then(|value| value.as_str())
            .map(|value| value.trim().to_ascii_lowercase())
        });

      if let Ok(mut cache) = self.package_type_by_dir.lock() {
        cache.insert(dir.to_path_buf(), package_type.clone());
      }

      if package_type.is_some() {
        return package_type;
      }

      current_dir = dir.parent();
    }

    None
  }

  fn resolve_browser_mapped_specifier(
    &self,
    specifier: &str,
    referrer: &str,
  ) -> Option<deno_core::ModuleSpecifier> {
    if !self.use_browser_mappings {
      return None;
    }

    let parent_nv = self.package_nv_for_referrer(referrer)?;
    let package_root = self.npm_cache.package_folder_for_nv(&parent_nv);
    let browser_field = std::fs::read_to_string(package_root.join("package.json"))
      .ok()
      .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
      .and_then(|json| json.get("browser").cloned())?;
    let browser_map = browser_field.as_object()?;

    let lookup_key = if specifier.starts_with("./") || specifier.starts_with("../") {
      let referrer_specifier = deno_core::ModuleSpecifier::parse(referrer).ok()?;
      let referrer_path = referrer_specifier.to_file_path().ok()?;
      let candidate_path = referrer_path
        .parent()
        .map(|parent| parent.join(specifier))
        .unwrap_or_else(|| PathBuf::from(specifier));
      let normalized = deno_path_util::normalize_path(candidate_path.into());
      let relative = normalized.as_ref().strip_prefix(&package_root).ok()?;
      let relative = relative.to_string_lossy().replace('\\', "/");
      format!("./{relative}")
    } else {
      specifier.to_string()
    };

    let mapped_entry = browser_map.get(&lookup_key)?;
    if mapped_entry == &serde_json::Value::Bool(false) {
      return None;
    }
    let mapped = mapped_entry.as_str()?;
    if mapped.starts_with("./") {
      let sub_path = mapped.trim_start_matches("./").trim_start_matches('/');
      return deno_core::ModuleSpecifier::parse(&format!(
        "npm:{}@{}/{}",
        parent_nv.name, parent_nv.version, sub_path
      ))
      .ok();
    }
    if mapped.starts_with("npm:")
      || mapped.starts_with("jsr:")
      || mapped.starts_with("node:")
      || mapped.starts_with("file:")
      || mapped.starts_with("http:")
      || mapped.starts_with("https:")
    {
      return deno_core::ModuleSpecifier::parse(mapped).ok();
    }

    if let Some((dep_name, dep_sub_path)) = Self::parse_bare_dependency_specifier(mapped) {
      let dep_specifier =
        self.resolve_dependency_request_for_parent(&parent_nv, &dep_name, dep_sub_path.as_deref());
      return deno_core::ModuleSpecifier::parse(&dep_specifier).ok();
    }

    None
  }

  fn package_nv_for_referrer(&self, referrer: &str) -> Option<PackageNv> {
    let referrer_specifier = deno_core::ModuleSpecifier::parse(referrer).ok()?;
    if referrer_specifier.scheme() != "file" {
      return None;
    }
    let referrer_path = referrer_specifier.to_file_path().ok()?;

    let guard = self.package_nv_by_folder.lock().ok()?;
    let mut best: Option<(usize, PackageNv)> = None;

    for (folder, package_nv) in guard.iter() {
      if !referrer_path.starts_with(folder) {
        continue;
      }
      let depth = folder.components().count();
      match best {
        Some((best_depth, _)) if best_depth >= depth => {}
        _ => best = Some((depth, package_nv.clone())),
      }
    }

    best.map(|(_, package_nv)| package_nv)
  }

  fn resolve_dependency_request_for_parent(
    &self,
    parent: &PackageNv,
    dep_name: &str,
    sub_path: Option<&str>,
  ) -> String {
    let maybe_dep_nv = self
      .dependency_nv_by_parent
      .lock()
      .ok()
      .and_then(|map| map.get(parent).cloned())
      .and_then(|deps| deps.get(dep_name).cloned());

    match (maybe_dep_nv, sub_path) {
      (Some(dep_nv), Some(sub_path)) if !sub_path.is_empty() => {
        format!("npm:{}@{}/{}", dep_nv.name, dep_nv.version, sub_path)
      }
      (Some(dep_nv), _) => format!("npm:{}@{}", dep_nv.name, dep_nv.version),
      (None, Some(sub_path)) if !sub_path.is_empty() => format!("npm:{dep_name}/{sub_path}"),
      (None, _) => format!("npm:{dep_name}"),
    }
  }

  fn resolve_candidate_path(package_root: &Path, value: &str) -> Option<PathBuf> {
    let candidate = value.trim().trim_start_matches("./");
    if candidate.is_empty() {
      return None;
    }

    let target = package_root.join(candidate);
    if target.is_file() {
      return Some(target);
    }

    if target.is_dir() {
      for index_name in ["index.mjs", "index.js", "index.cjs"] {
        let index_path = target.join(index_name);
        if index_path.is_file() {
          return Some(index_path);
        }
      }
    }

    if target.extension().is_none() {
      for ext in ["mjs", "js", "cjs"] {
        let with_ext = target.with_extension(ext);
        if with_ext.is_file() {
          return Some(with_ext);
        }
      }
    }

    None
  }

  fn collect_exports_candidates(
    value: &serde_json::Value,
    conditions: &[&str],
    out: &mut Vec<String>,
  ) {
    match value {
      serde_json::Value::String(text) => out.push(text.to_string()),
      serde_json::Value::Object(map) => {
        if let Some(root_entry) = map.get(".") {
          Self::collect_exports_candidates(root_entry, conditions, out);
        }
        for condition in conditions {
          if let Some(entry) = map.get(*condition) {
            Self::collect_exports_candidates(entry, conditions, out);
          }
        }
      }
      _ => {}
    }
  }

  fn exports_conditions(&self) -> &'static [&'static str] {
    if self.use_browser_mappings {
      &[
        "deno", "browser", "import", "module", "default", "require", "node",
      ]
    } else {
      &["deno", "import", "module", "default", "node", "require"]
    }
  }

  fn resolve_package_entry_path(
    &self,
    package_nv: &PackageNv,
    sub_path: Option<&str>,
  ) -> Result<PathBuf, ModuleLoaderError> {
    let package_root = self.npm_cache.package_folder_for_nv(package_nv);

    if let Some(sub_path) = sub_path {
      let sub_path = sub_path.trim_start_matches('/');
      if !sub_path.is_empty() {
        if let Some(path) = Self::resolve_candidate_path(&package_root, sub_path) {
          return Ok(path);
        }
      }
    }

    let package_json_path = package_root.join("package.json");
    let package_json: serde_json::Value = std::fs::read_to_string(&package_json_path)
      .ok()
      .and_then(|text| serde_json::from_str(&text).ok())
      .unwrap_or_else(|| serde_json::json!({}));

    let mut candidates = Vec::new();
    if let Some(exports) = package_json.get("exports") {
      let export_conditions = self.exports_conditions();
      Self::collect_exports_candidates(exports, export_conditions, &mut candidates);
    }
    if self.use_browser_mappings {
      if let Some(browser) = package_json.get("browser").and_then(|value| value.as_str()) {
        candidates.push(browser.to_string());
      }
    }
    if let Some(module) = package_json.get("module").and_then(|value| value.as_str()) {
      candidates.push(module.to_string());
    }
    if let Some(main) = package_json.get("main").and_then(|value| value.as_str()) {
      candidates.push(main.to_string());
    }
    candidates.push("index.js".to_string());

    let mut dedup = HashSet::new();
    for candidate in candidates {
      if !dedup.insert(candidate.clone()) {
        continue;
      }
      if let Some(path) = Self::resolve_candidate_path(&package_root, &candidate) {
        return Ok(path);
      }
    }

    Err(JsErrorBox::generic(format!(
      "failed to resolve npm package entry for `{}` in {}",
      package_nv,
      package_root.display()
    )))
  }

  fn ensure_req_and_dependencies<'a>(
    &'a self,
    req: PackageReq,
    visited: &'a mut HashSet<PackageNv>,
  ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<PackageNv, ModuleLoaderError>> + 'a>>
  {
    Box::pin(async move {
      let package_info = self
        .registry_info_provider
        .package_info(req.name.as_str())
        .await
        .map_err(|err| {
          JsErrorBox::generic(format!(
            "failed to load npm package metadata for `{}`: {err}",
            req.name
          ))
        })?;

      let version_info = self
        .version_resolver
        .get_for_package(&package_info)
        .resolve_best_package_version_info(&req.version_req, std::iter::empty())
        .map_err(|err| {
          JsErrorBox::generic(format!(
            "failed to resolve npm version for `{}` with `{}`: {err}",
            req.name,
            req.version_req.version_text()
          ))
        })?;

      let package_nv = PackageNv {
        name: req.name.clone(),
        version: version_info.version.clone(),
      };

      if !visited.insert(package_nv.clone()) {
        return Ok(package_nv);
      }

      let dist: NpmPackageVersionDistInfo = version_info.dist.clone().ok_or_else(|| {
        JsErrorBox::generic(format!(
          "npm package `{}` missing dist metadata for version `{}`",
          package_nv.name, package_nv.version
        ))
      })?;

      self
        .tarball_cache
        .ensure_package(&package_nv, &dist)
        .await
        .map_err(|err| {
          JsErrorBox::generic(format!(
            "failed to cache npm package `{}`: {err}",
            package_nv
          ))
        })?;

      let package_folder = self.npm_cache.package_folder_for_nv(&package_nv);
      if let Ok(mut map) = self.package_nv_by_folder.lock() {
        map.insert(package_folder, package_nv.clone());
      }

      let dep_entries = version_info
        .dependencies_as_entries(req.name.as_str())
        .map_err(|err| {
          JsErrorBox::generic(format!(
            "failed to parse dependencies for npm package `{}`: {err}",
            package_nv
          ))
        })?;

      let mut resolved_dependencies = HashMap::new();
      for dep_entry in dep_entries {
        let dep_req = PackageReq {
          name: dep_entry.name.clone(),
          version_req: dep_entry.version_req,
        };

        let dep_nv = self.ensure_req_and_dependencies(dep_req, visited).await?;
        resolved_dependencies.insert(dep_entry.name.to_string(), dep_nv);
      }

      if let Ok(mut map) = self.dependency_nv_by_parent.lock() {
        map.insert(package_nv.clone(), resolved_dependencies);
      }

      Ok(package_nv)
    })
  }

  async fn resolve_npm_specifier_to_file(
    &self,
    npm_specifier: &str,
  ) -> Result<deno_core::ModuleSpecifier, ModuleLoaderError> {
    if let Ok(cache) = self.resolved_specifier_cache.lock() {
      if let Some(found) = cache.get(npm_specifier) {
        return Ok(found.clone());
      }
    }

    let req_ref = NpmPackageReqReference::from_str(npm_specifier).map_err(|err| {
      JsErrorBox::generic(format!("invalid npm specifier `{npm_specifier}`: {err}"))
    })?;

    let mut visited = HashSet::new();
    let package_nv = self
      .ensure_req_and_dependencies(req_ref.req().clone(), &mut visited)
      .await?;

    let entry_path = self.resolve_package_entry_path(&package_nv, req_ref.sub_path())?;
    let resolved_specifier =
      deno_core::ModuleSpecifier::from_file_path(&entry_path).map_err(|_| {
        JsErrorBox::generic(format!(
          "failed converting npm entry path to file specifier: {}",
          entry_path.display()
        ))
      })?;

    if let Ok(mut cache) = self.resolved_specifier_cache.lock() {
      cache.insert(npm_specifier.to_string(), resolved_specifier.clone());
    }

    Ok(resolved_specifier)
  }
}

#[derive(Debug, Default)]
struct JsrPackageResolver {
  version_resolver: JsrVersionResolver,
  package_info_cache: Mutex<HashMap<String, Arc<JsrPackageInfo>>>,
  package_version_info_cache: Mutex<HashMap<PackageNv, Arc<JsrPackageVersionInfo>>>,
  resolved_specifier_cache: Mutex<HashMap<String, deno_core::ModuleSpecifier>>,
}

impl JsrPackageResolver {
  fn new() -> Self {
    Self::default()
  }

  fn create_http_agent() -> ureq::Agent {
    configured_ureq_agent()
  }

  fn fetch_json<T: serde::de::DeserializeOwned>(
    &self,
    url: &str,
    resource_name: &str,
  ) -> Result<T, ModuleLoaderError> {
    if let Some(cached) = read_cached_bytes("jsr_meta", url) {
      if let Ok(parsed) = serde_json::from_slice::<T>(&cached) {
        return Ok(parsed);
      }
    }

    let agent = Self::create_http_agent();
    let mut retried = false;

    let bytes = loop {
      let response = match agent
        .get(url)
        .header(
          "user-agent",
          "libmainworker_duplex_stream_example/jsr-resolver",
        )
        .header("accept", "application/json")
        .call()
      {
        Ok(response) => response,
        Err(err) if !retried => {
          retried = true;
          std::thread::sleep(Duration::from_millis(50));
          eprintln!("retrying jsr metadata fetch after transient error: {url} ({err})");
          continue;
        }
        Err(err) => {
          return Err(JsErrorBox::generic(format!(
            "failed to fetch jsr metadata `{resource_name}` ({url}): {err}"
          )));
        }
      };

      match response
        .into_body()
        .with_config()
        .limit(1024 * 1024 * 16)
        .read_to_vec()
      {
        Ok(bytes) => break bytes,
        Err(err) if !retried => {
          retried = true;
          std::thread::sleep(Duration::from_millis(50));
          eprintln!("retrying jsr metadata body read after transient error: {url} ({err})");
        }
        Err(err) => {
          return Err(JsErrorBox::generic(format!(
            "failed to read jsr metadata `{resource_name}` body ({url}): {err}"
          )));
        }
      }
    };

    let parsed = serde_json::from_slice::<T>(&bytes).map_err(|err| {
      let snippet = String::from_utf8_lossy(&bytes[.. bytes.len().min(120)]);
      JsErrorBox::generic(format!(
        "failed to parse jsr metadata `{resource_name}` from {url}: {err}; body starts with: \
         {snippet}"
      ))
    })?;
    write_cached_bytes("jsr_meta", url, &bytes);
    Ok(parsed)
  }

  fn package_info(&self, package_name: &str) -> Result<Arc<JsrPackageInfo>, ModuleLoaderError> {
    if let Ok(cache) = self.package_info_cache.lock() {
      if let Some(info) = cache.get(package_name) {
        return Ok(info.clone());
      }
    }

    let meta_url = format!("https://jsr.io/{package_name}/meta.json");
    let info = Arc::new(self.fetch_json::<JsrPackageInfo>(&meta_url, package_name)?);

    if let Ok(mut cache) = self.package_info_cache.lock() {
      cache.insert(package_name.to_string(), info.clone());
    }

    Ok(info)
  }

  fn package_version_info(
    &self,
    package_nv: &PackageNv,
  ) -> Result<Arc<JsrPackageVersionInfo>, ModuleLoaderError> {
    if let Ok(cache) = self.package_version_info_cache.lock() {
      if let Some(info) = cache.get(package_nv) {
        return Ok(info.clone());
      }
    }

    let meta_url = format!(
      "https://jsr.io/{}/{}_meta.json",
      package_nv.name, package_nv.version
    );
    let info =
      Arc::new(self.fetch_json::<JsrPackageVersionInfo>(&meta_url, &package_nv.to_string())?);

    if let Ok(mut cache) = self.package_version_info_cache.lock() {
      cache.insert(package_nv.clone(), info.clone());
    }

    Ok(info)
  }

  fn export_to_module_specifier(
    package_nv: &PackageNv,
    export_target: &str,
  ) -> Result<deno_core::ModuleSpecifier, ModuleLoaderError> {
    if export_target.starts_with("https://") || export_target.starts_with("http://") {
      return deno_core::ModuleSpecifier::parse(export_target).map_err(JsErrorBox::from_err);
    }

    let module_path = export_target
      .strip_prefix("./")
      .unwrap_or(export_target)
      .trim_start_matches('/');
    if module_path.is_empty() {
      return Err(JsErrorBox::generic(format!(
        "invalid empty jsr export target for `{}`",
        package_nv
      )));
    }

    deno_core::ModuleSpecifier::parse(&format!(
      "https://jsr.io/{}/{}/{}",
      package_nv.name, package_nv.version, module_path
    ))
    .map_err(JsErrorBox::from_err)
  }

  fn resolve_jsr_specifier_to_https(
    &self,
    jsr_specifier: &str,
  ) -> Result<deno_core::ModuleSpecifier, ModuleLoaderError> {
    if let Ok(cache) = self.resolved_specifier_cache.lock() {
      if let Some(found) = cache.get(jsr_specifier) {
        return Ok(found.clone());
      }
    }

    let req_ref = JsrPackageReqReference::from_str(jsr_specifier).map_err(|err| {
      JsErrorBox::generic(format!("invalid jsr specifier `{jsr_specifier}`: {err}"))
    })?;

    let req = req_ref.req().clone();
    let package_info = self.package_info(req.name.as_str())?;
    let version_resolver = self
      .version_resolver
      .get_for_package(&req.name, &package_info);
    let resolved_version = version_resolver
      .resolve_version(&req, std::iter::empty())
      .map_err(|err| {
        JsErrorBox::generic(format!(
          "failed to resolve jsr package version for `{}`: {err}",
          req
        ))
      })?;

    let package_nv = PackageNv {
      name: req.name.clone(),
      version: resolved_version.version.clone(),
    };

    let version_info = self.package_version_info(&package_nv)?;
    let export_name = req_ref.export_name();
    let export_target = if let Some(export_target) = version_info.export(export_name.as_ref()) {
      export_target.to_string()
    } else if let Some(sub_path) = req_ref.sub_path() {
      format!("./{}", sub_path.trim_start_matches('/'))
    } else {
      return Err(JsErrorBox::generic(format!(
        "jsr package `{}` version `{}` does not expose export `{}`",
        package_nv.name, package_nv.version, export_name
      )));
    };

    let module_specifier = Self::export_to_module_specifier(&package_nv, &export_target)?;

    if let Ok(mut cache) = self.resolved_specifier_cache.lock() {
      cache.insert(jsr_specifier.to_string(), module_specifier.clone());
    }

    Ok(module_specifier)
  }
}

pub(crate) struct DirectModuleLoader {
  source_maps: SourceMapStore,
  npm_resolver: Arc<NpmPackageResolver>,
  jsr_resolver: Arc<JsrPackageResolver>,
}

impl DirectModuleLoader {
  pub(crate) fn new() -> Self {
    let npm_resolver = NpmPackageResolver::new().unwrap_or_else(|err| {
      panic!("failed to initialize npm package resolver: {err}");
    });
    let jsr_resolver = JsrPackageResolver::new();

    Self {
      source_maps: Rc::new(RefCell::new(HashMap::new())),
      npm_resolver: Arc::new(npm_resolver),
      jsr_resolver: Arc::new(jsr_resolver),
    }
  }

  fn module_kind(media_type: MediaType) -> Result<(ModuleType, bool), ModuleLoaderError> {
    match media_type {
      MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
        Ok((ModuleType::JavaScript, false))
      }
      MediaType::Jsx => Ok((ModuleType::JavaScript, true)),
      MediaType::TypeScript
      | MediaType::Mts
      | MediaType::Cts
      | MediaType::Dts
      | MediaType::Dmts
      | MediaType::Dcts
      | MediaType::Tsx => Ok((ModuleType::JavaScript, true)),
      MediaType::Json => Ok((ModuleType::Json, false)),
      MediaType::Unknown => Ok((ModuleType::JavaScript, false)),
      _ => Err(JsErrorBox::generic(format!(
        "unsupported media type: {media_type:?}"
      ))),
    }
  }

  fn transpile_if_needed(
    source_maps: SourceMapStore,
    module_specifier: &deno_core::ModuleSpecifier,
    code: String,
    media_type: MediaType,
    should_transpile: bool,
  ) -> Result<String, ModuleLoaderError> {
    if !should_transpile {
      return Ok(code);
    }

    let parsed = deno_ast::parse_module(ParseParams {
      specifier: module_specifier.clone(),
      text: code.into(),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;

    let transpiled = parsed
      .transpile(
        &deno_ast::TranspileOptions {
          imports_not_used_as_values: deno_ast::ImportsNotUsedAsValues::Remove,
          decorators: deno_ast::DecoratorsTranspileOption::Ecma,
          ..Default::default()
        },
        &deno_ast::TranspileModuleOptions::default(),
        &deno_ast::EmitOptions {
          source_map: SourceMapOption::Separate,
          inline_sources: true,
          ..Default::default()
        },
      )
      .map_err(|err| JsErrorBox::generic(err.to_string()))?
      .into_source();

    if let Some(source_map) = transpiled.source_map {
      source_maps
        .borrow_mut()
        .insert(module_specifier.to_string(), source_map.into_bytes());
    }

    String::from_utf8(transpiled.text.into_bytes())
      .map_err(|err| JsErrorBox::generic(err.to_string()))
  }

  fn is_valid_export_ident(name: &str) -> bool {
    if name.is_empty() {
      return false;
    }
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
      return false;
    };
    if !first.is_ascii_alphabetic() && first != '_' && first != '$' {
      return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
  }

  fn append_named_export(
    out: &mut String,
    export_name: &str,
    quoted_export_name: &str,
    initializer: &str,
    temp_var_count: &mut usize,
  ) {
    if JS_RESERVED_WORDS.contains(export_name) || !Self::is_valid_export_ident(export_name) {
      *temp_var_count += 1;
      out.push_str("const __deno_export_");
      out.push_str(&temp_var_count.to_string());
      out.push_str("__ = ");
      out.push_str(initializer);
      out.push_str(";\nexport { __deno_export_");
      out.push_str(&temp_var_count.to_string());
      out.push_str("__ as ");
      out.push_str(quoted_export_name);
      out.push_str(" };\n");
    } else {
      out.push_str("export const ");
      out.push_str(export_name);
      out.push_str(" = ");
      out.push_str(initializer);
      out.push_str(";\n");
    }
  }

  fn build_commonjs_wrapper(
    entry_path: &Path,
    source_code: &str,
    export_names: &BTreeSet<String>,
  ) -> String {
    let mut out = String::new();
    let entry_path_text = entry_path.to_string_lossy().to_string();
    let entry_dir_text = entry_path
      .parent()
      .map(|path| path.to_string_lossy().to_string())
      .unwrap_or_default();
    let quoted_entry = to_double_quote_string(&entry_path_text);
    let quoted_entry_dir = to_double_quote_string(&entry_dir_text);

    out.push_str("const __filename = ");
    out.push_str(&quoted_entry);
    out.push_str(";\n");
    out.push_str("const __dirname = ");
    out.push_str(&quoted_entry_dir);
    out.push_str(";\n");
    out.push_str("const __cjsModule = { exports: {} };\n");
    out.push_str("const __cjsExports = __cjsModule.exports;\n");
    out.push_str("const __cjsRequire = (specifier) => {\n");
    out.push_str(
      "  throw new Error(`CommonJS require(${specifier}) is not supported for ${__filename}`);\n",
    );
    out.push_str("};\n");
    out.push_str("(function (module, exports, require, __filename, __dirname) {\n");
    out.push_str(source_code);
    out.push_str("\n})(__cjsModule, __cjsExports, __cjsRequire, __filename, __dirname);\n");
    out.push_str("const mod = __cjsModule.exports;\n");

    let mut temp_var_count = 0_usize;
    for export_name in export_names {
      if export_name == "default" || export_name == "module.exports" {
        continue;
      }
      let quoted = to_double_quote_string(export_name);
      let initializer = format!("mod[{quoted}]");
      Self::append_named_export(
        &mut out,
        export_name,
        &quoted,
        &initializer,
        &mut temp_var_count,
      );
    }

    out.push_str("export default mod;\n");
    Self::append_named_export(
      &mut out,
      "module.exports",
      "\"module.exports\"",
      "mod",
      &mut temp_var_count,
    );
    out
  }

  fn analyze_commonjs_export_names(
    module_specifier: &deno_core::ModuleSpecifier,
    media_type: MediaType,
    code: &str,
  ) -> Result<BTreeSet<String>, ModuleLoaderError> {
    let parse_media_type = if matches!(media_type, MediaType::Cjs) {
      MediaType::Cjs
    } else {
      MediaType::JavaScript
    };
    let parsed = deno_ast::parse_program(ParseParams {
      specifier: module_specifier.clone(),
      text: code.to_string().into(),
      media_type: parse_media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;

    let analysis = parsed.analyze_cjs();
    Ok(analysis.exports.into_iter().collect())
  }

  fn should_wrap_as_commonjs(
    npm_resolver: &NpmPackageResolver,
    module_specifier: &deno_core::ModuleSpecifier,
    path: &Path,
    media_type: MediaType,
    code: &str,
  ) -> Result<bool, ModuleLoaderError> {
    if !npm_resolver.is_in_npm_cache_path(path) {
      return Ok(false);
    }
    if matches!(media_type, MediaType::Mjs | MediaType::Mts) {
      return Ok(false);
    }
    if matches!(media_type, MediaType::Cjs) {
      return Ok(true);
    }
    if !matches!(media_type, MediaType::JavaScript | MediaType::Unknown) {
      return Ok(false);
    }

    let package_type = npm_resolver.package_type_for_module_path(path);
    if matches!(package_type.as_deref(), Some("module")) {
      return Ok(false);
    }
    if matches!(package_type.as_deref(), Some("commonjs")) {
      return Ok(true);
    }

    let parsed = deno_ast::parse_program(ParseParams {
      specifier: module_specifier.clone(),
      text: code.to_string().into(),
      media_type: MediaType::JavaScript,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;
    Ok(parsed.program_ref().compute_is_script())
  }

  fn load_file_module(
    source_maps: SourceMapStore,
    module_specifier: &deno_core::ModuleSpecifier,
    npm_resolver: Option<&NpmPackageResolver>,
  ) -> Result<ModuleSource, ModuleLoaderError> {
    let path = module_specifier.to_file_path().map_err(|_| {
      JsErrorBox::generic("there was an error converting the module specifier to a file path")
    })?;
    let media_type = MediaType::from_path(&path);
    let mut code =
      std::fs::read_to_string(&path).map_err(|err| JsErrorBox::generic(err.to_string()))?;
    code = Self::rewrite_disabled_commonjs_stubs(module_specifier, &code);

    if let Some(npm_resolver) = npm_resolver {
      if Self::should_wrap_as_commonjs(npm_resolver, module_specifier, &path, media_type, &code)? {
        let export_names =
          Self::analyze_commonjs_export_names(module_specifier, media_type, &code)?;
        let wrapper = Self::build_commonjs_wrapper(&path, &code, &export_names);
        return Ok(ModuleSource::new(
          ModuleType::JavaScript,
          ModuleSourceCode::String(wrapper.into()),
          module_specifier,
          None,
        ));
      }
    }

    let (module_type, should_transpile) = Self::module_kind(media_type)?;
    let code = Self::transpile_if_needed(
      source_maps,
      module_specifier,
      code,
      media_type,
      should_transpile,
    )?;

    Ok(ModuleSource::new(
      module_type,
      ModuleSourceCode::String(code.into()),
      module_specifier,
      None,
    ))
  }

  fn rewrite_disabled_commonjs_stubs(
    module_specifier: &deno_core::ModuleSpecifier,
    code: &str,
  ) -> String {
    if module_specifier.scheme() != "file" {
      return code.to_string();
    }

    let Ok(path) = module_specifier.to_file_path() else {
      return code.to_string();
    };
    let path_text = path.to_string_lossy().to_string();
    if !path_text.contains("/npm/registry.npmjs.org/") {
      return code.to_string();
    }

    let mut working_code = code.to_string();
    let mut changed = false;

    let replaced_form_data =
      FORM_DATA_IMPORT_RE.replace_all(&working_code, "const $1 = globalThis.FormData;");
    if replaced_form_data.as_ref() != working_code {
      working_code = replaced_form_data.into_owned();
      changed = true;
    }

    if !working_code.contains("(disabled):") {
      return working_code;
    }

    let mut imports = Vec::<&'static str>::new();
    let mut seen_imports = HashSet::<&'static str>::new();
    let mut needs_jsonwebtoken_shim = false;

    let rewritten =
      DISABLED_COMMONJS_STUB_RE.replace_all(&working_code, |caps: &regex::Captures| {
        let Some(var_name) = caps.name("var_name").map(|m| m.as_str()) else {
          return caps
            .get(0)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        };
        let Some(label) = caps.name("label").map(|m| m.as_str()) else {
          return caps
            .get(0)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        };

        let replacement_target = match label {
          "https" | "node:https" => {
            let import_stmt = "import * as __libmw_node_https from \"node:https\";";
            if seen_imports.insert(import_stmt) {
              imports.push(import_stmt);
            }
            Some("__libmw_node_https")
          }
          "http" | "node:http" => {
            let import_stmt = "import * as __libmw_node_http from \"node:http\";";
            if seen_imports.insert(import_stmt) {
              imports.push(import_stmt);
            }
            Some("__libmw_node_http")
          }
          "crypto" | "node:crypto" => {
            let import_stmt = "import * as __libmw_node_crypto from \"node:crypto\";";
            if seen_imports.insert(import_stmt) {
              imports.push(import_stmt);
            }
            Some("__libmw_node_crypto")
          }
          "node_modules/jsonwebtoken/index.js" | "jsonwebtoken" => {
            let crypto_import = "import * as __libmw_node_crypto from \"node:crypto\";";
            if seen_imports.insert(crypto_import) {
              imports.push(crypto_import);
            }
            let buffer_import = "import { Buffer as __libmw_Buffer } from \"node:buffer\";";
            if seen_imports.insert(buffer_import) {
              imports.push(buffer_import);
            }
            needs_jsonwebtoken_shim = true;
            Some("__libmw_jsonwebtoken_shim")
          }
          "node_modules/ws/index.js" | "ws" => {
            let import_stmt = "import __libmw_npm_ws_default from \"ws\";";
            if seen_imports.insert(import_stmt) {
              imports.push(import_stmt);
            }
            Some("__libmw_npm_ws_default")
          }
          _ => None,
        };

        if let Some(target) = replacement_target {
          changed = true;
          format!("var {var_name} = () => {target};\n")
        } else {
          caps
            .get(0)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default()
        }
      });

    if !changed {
      return working_code;
    }

    let mut out = String::new();
    for import in imports {
      out.push_str(import);
      out.push('\n');
    }
    if needs_jsonwebtoken_shim {
      out.push_str(
        "const __libmwBase64UrlEncode = (value) => __libmw_Buffer.from(value)\n  \
         .toString(\"base64\")\n  .replace(/\\+/g, \"-\")\n  .replace(/\\//g, \"_\")\n  \
         .replace(/=+$/g, \"\");\nconst __libmw_jsonwebtoken_shim = {\n  sign(payload, secret, \
         options = {}) {\n    const algorithm = options.algorithm ?? \"HS256\";\n    if \
         (algorithm !== \"HS256\") {\n      throw new Error(`unsupported jwt algorithm: \
         ${algorithm}`);\n    }\n    const normalizedPayload = { ...(payload ?? {}) };\n    if \
         (!options.noTimestamp && normalizedPayload.iat == null) {\n      normalizedPayload.iat = \
         Math.floor(Date.now() / 1000);\n    }\n    const headerPart = \
         __libmwBase64UrlEncode(JSON.stringify({ alg: \"HS256\", typ: \"JWT\" }));\n    const \
         payloadPart = __libmwBase64UrlEncode(JSON.stringify(normalizedPayload));\n    const \
         signingInput = `${headerPart}.${payloadPart}`;\n    const signature = \
         __libmw_node_crypto\n      .createHmac(\"sha256\", String(secret ?? \"\"))\n      \
         .update(signingInput)\n      .digest(\"base64\")\n      .replace(/\\+/g, \"-\")\n      \
         .replace(/\\//g, \"_\")\n      .replace(/=+$/g, \"\");\n    return \
         `${signingInput}.${signature}`;\n  },\n};\n",
      );
    }
    out.push_str(rewritten.as_ref());
    out
  }

  fn load_https_module(
    source_maps: SourceMapStore,
    module_specifier: &deno_core::ModuleSpecifier,
  ) -> Result<ModuleSource, ModuleLoaderError> {
    let url = module_specifier.to_string();
    if let Some(cached_body) = read_cached_text("remote_modules", &url) {
      let media_type = MediaType::from_path(Path::new(module_specifier.path()));
      let (module_type, should_transpile) = Self::module_kind(media_type)?;
      let cached_body = Self::transpile_if_needed(
        source_maps,
        module_specifier,
        cached_body,
        media_type,
        should_transpile,
      )?;

      return Ok(ModuleSource::new(
        module_type,
        ModuleSourceCode::String(cached_body.into()),
        module_specifier,
        None,
      ));
    }

    let agent = configured_ureq_agent();
    let mut retried = false;
    let body = loop {
      let response = match agent
        .get(&url)
        .header(
          "user-agent",
          "libmainworker_duplex_stream_example/module-loader",
        )
        .call()
      {
        Ok(response) => response,
        Err(err) if !retried => {
          retried = true;
          std::thread::sleep(Duration::from_millis(50));
          eprintln!("retrying remote module fetch after transient error: {url} ({err})");
          continue;
        }
        Err(err) => {
          return Err(JsErrorBox::generic(format!(
            "failed to fetch remote module {url}: {err}"
          )));
        }
      };

      match response.into_body().read_to_string() {
        Ok(body) => break body,
        Err(err) if !retried => {
          retried = true;
          std::thread::sleep(Duration::from_millis(50));
          eprintln!("retrying remote module body read after transient error: {url} ({err})");
        }
        Err(err) => {
          return Err(JsErrorBox::generic(format!(
            "failed to read remote module body {url}: {err}"
          )));
        }
      }
    };
    write_cached_text("remote_modules", &url, &body);

    let media_type = MediaType::from_path(Path::new(module_specifier.path()));
    let (module_type, should_transpile) = Self::module_kind(media_type)?;
    let body = Self::transpile_if_needed(
      source_maps,
      module_specifier,
      body,
      media_type,
      should_transpile,
    )?;

    Ok(ModuleSource::new(
      module_type,
      ModuleSourceCode::String(body.into()),
      module_specifier,
      None,
    ))
  }

  fn maybe_node_default_compat_specifier(
    npm_resolver: &NpmPackageResolver,
    specifier: &str,
    referrer: &str,
  ) -> Option<deno_core::ModuleSpecifier> {
    if !matches!(specifier, "node:http" | "node:https") {
      return None;
    }
    if npm_resolver.package_nv_for_referrer(referrer).is_none() {
      return None;
    }

    let wrapper = format!(
      "import * as __m from \"{specifier}\";\nexport default __m;\nexport * from \
       \"{specifier}\";\n"
    );
    let encoded = base64::engine::general_purpose::STANDARD.encode(wrapper);
    deno_core::ModuleSpecifier::parse(&format!("data:application/javascript;base64,{encoded}")).ok()
  }

  fn load_data_module(
    module_specifier: &deno_core::ModuleSpecifier,
  ) -> Result<ModuleSource, ModuleLoaderError> {
    let raw = module_specifier.as_str();
    let encoded = raw
      .strip_prefix("data:application/javascript;base64,")
      .ok_or_else(|| {
        JsErrorBox::generic(format!(
          "unsupported data module format: {module_specifier}"
        ))
      })?;
    let decoded = base64::engine::general_purpose::STANDARD
      .decode(encoded)
      .map_err(|err| JsErrorBox::generic(format!("invalid data module base64: {err}")))?;
    let code = String::from_utf8(decoded)
      .map_err(|err| JsErrorBox::generic(format!("invalid utf-8 in data module: {err}")))?;
    Ok(ModuleSource::new(
      ModuleType::JavaScript,
      ModuleSourceCode::String(code.into()),
      module_specifier,
      None,
    ))
  }
}

impl ModuleLoader for DirectModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<deno_core::ModuleSpecifier, ModuleLoaderError> {
    if trace_module_loader_enabled() {
      eprintln!("[module_loader][resolve] specifier={specifier} referrer={referrer}");
    }
    if specifier.starts_with("node:") {
      if let Some(compat) =
        Self::maybe_node_default_compat_specifier(self.npm_resolver.as_ref(), specifier, referrer)
      {
        return Ok(compat);
      }
      return deno_core::ModuleSpecifier::parse(specifier).map_err(JsErrorBox::from_err);
    }
    if deno_runtime::deno_node::is_builtin_node_module(specifier) {
      let node_specifier = format!("node:{specifier}");
      if let Some(compat) = Self::maybe_node_default_compat_specifier(
        self.npm_resolver.as_ref(),
        &node_specifier,
        referrer,
      ) {
        return Ok(compat);
      }
      return deno_core::ModuleSpecifier::parse(&node_specifier).map_err(JsErrorBox::from_err);
    }
    if specifier.starts_with("npm:") {
      return NpmPackageResolver::parse_npm_specifier(specifier);
    }

    if specifier.starts_with("jsr:") {
      return self.jsr_resolver.resolve_jsr_specifier_to_https(specifier);
    }

    if let Some(mapped) = self
      .npm_resolver
      .resolve_browser_mapped_specifier(specifier, referrer)
    {
      return Ok(mapped);
    }

    if let Some((dep_name, dep_sub_path)) =
      NpmPackageResolver::parse_bare_dependency_specifier(specifier)
    {
      if let Some(parent_nv) = self.npm_resolver.package_nv_for_referrer(referrer) {
        let dep_specifier = self.npm_resolver.resolve_dependency_request_for_parent(
          &parent_nv,
          &dep_name,
          dep_sub_path.as_deref(),
        );
        return NpmPackageResolver::parse_npm_specifier(&dep_specifier);
      }
    }

    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    module_specifier: &deno_core::ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();
    let module_specifier_text = module_specifier.to_string();

    let result = match module_specifier.scheme() {
      "file" => ModuleLoadResponse::Sync(Self::load_file_module(
        self.source_maps.clone(),
        &module_specifier,
        Some(self.npm_resolver.as_ref()),
      )),
      "https" => ModuleLoadResponse::Sync(Self::load_https_module(
        self.source_maps.clone(),
        &module_specifier,
      )),
      "data" => ModuleLoadResponse::Sync(Self::load_data_module(&module_specifier)),
      "jsr" => {
        let jsr_resolver = self.jsr_resolver.clone();
        let source_maps = self.source_maps.clone();
        let result = jsr_resolver
          .resolve_jsr_specifier_to_https(module_specifier.as_str())
          .and_then(|remote_specifier| Self::load_https_module(source_maps, &remote_specifier));
        ModuleLoadResponse::Sync(result)
      }
      "npm" => {
        let source_maps = self.source_maps.clone();
        let npm_resolver = self.npm_resolver.clone();

        ModuleLoadResponse::Async(Box::pin(async move {
          let npm_specifier = module_specifier.to_string();

          let resolved_specifier = npm_resolver
            .resolve_npm_specifier_to_file(&npm_specifier)
            .await?;
          let loaded = Self::load_file_module(
            source_maps,
            &resolved_specifier,
            Some(npm_resolver.as_ref()),
          )?;
          Ok(ModuleSource::new_with_redirect(
            loaded.module_type,
            loaded.code,
            &module_specifier,
            &resolved_specifier,
            loaded.code_cache,
          ))
        }))
      }
      scheme => ModuleLoadResponse::Sync(Err(JsErrorBox::generic(format!(
        "unsupported module scheme: {scheme}"
      )))),
    };

    if trace_module_loader_enabled() {
      eprintln!("[module_loader][load] {module_specifier_text}");
    }

    result
  }

  fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
    self
      .source_maps
      .borrow()
      .get(specifier)
      .map(|source_map| Cow::Owned(source_map.clone()))
  }
}
