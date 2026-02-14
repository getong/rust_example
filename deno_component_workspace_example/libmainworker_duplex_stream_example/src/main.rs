use std::{
  borrow::Cow,
  cell::RefCell,
  collections::HashMap,
  path::PathBuf,
  rc::Rc,
  sync::{Arc, Mutex},
};

use deno_ast::{MediaType, ParseParams, SourceMapOption};
use deno_core::{
  ExtensionFileSource, ModuleLoadOptions, ModuleLoadReferrer, ModuleLoadResponse, ModuleLoader,
  ModuleSource, ModuleSourceCode, ModuleType, OpState, ResolutionKind, Resource, ResourceId,
  error::{AnyError, ModuleLoaderError},
  op2, resolve_import,
  url::Url,
};
use deno_error::JsErrorBox;
use deno_resolver::npm::{DenoInNpmPackageChecker, NpmResolver};
use deno_runtime::{
  BootstrapOptions, WorkerExecutionMode,
  ops::bootstrap::SnapshotOptions,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use tokio::{
  sync::mpsc,
  time::{Duration, MissedTickBehavior},
};

struct DuplexChannelPair {
  inbound_rx: mpsc::Receiver<String>,
  outbound_tx: mpsc::Sender<String>,
}

const DUPLEX_API_SPECIFIER: &str = "ext:libmainworker_duplex_ext/duplex_api.ts";
const EMBED_RESULT_SPECIFIER: &str = "ext:libmainworker_embed_ext/embed_result.ts";
const DUPLEX_API_SOURCE: &str = include_str!("duplex_api.ts");
const EMBED_RESULT_SOURCE: &str = include_str!("embed_result.ts");
const SHIM_MODULES_SOURCE: &str = include_str!("shim_modules.ts");

type SourceMapStore = Rc<RefCell<HashMap<String, Vec<u8>>>>;

#[derive(Clone)]
struct DuplexChannelSlot {
  channels: Arc<Mutex<Option<DuplexChannelPair>>>,
}

#[derive(Clone, Default)]
struct EmbedResult {
  result: Option<String>,
  exit_data: Option<String>,
}

#[derive(Debug)]
struct TokioDuplexResource {
  inbound_rx: tokio::sync::Mutex<mpsc::Receiver<String>>,
  outbound_tx: mpsc::Sender<String>,
}

impl Resource for TokioDuplexResource {
  fn name(&self) -> Cow<'_, str> {
    "mainworkerDuplex".into()
  }
}

struct DirectModuleLoader {
  source_maps: SourceMapStore,
}

impl DirectModuleLoader {
  fn new() -> Self {
    Self {
      source_maps: Rc::new(RefCell::new(HashMap::new())),
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

  fn load_file_module(
    source_maps: SourceMapStore,
    module_specifier: &deno_core::ModuleSpecifier,
  ) -> Result<ModuleSource, ModuleLoaderError> {
    let path = module_specifier.to_file_path().map_err(|_| {
      JsErrorBox::generic("there was an error converting the module specifier to a file path")
    })?;
    let media_type = MediaType::from_path(&path);
    let (module_type, should_transpile) = Self::module_kind(media_type)?;
    let code =
      std::fs::read_to_string(&path).map_err(|err| JsErrorBox::generic(err.to_string()))?;
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

  fn parse_npm_package_name(raw_specifier: &str) -> String {
    if raw_specifier.starts_with('@') {
      let mut it = raw_specifier.splitn(3, '/');
      let scope = it.next().unwrap_or_default();
      let name_and_rest = it.next().unwrap_or_default();
      let base_name = name_and_rest.split('@').next().unwrap_or(name_and_rest);
      format!("{scope}/{base_name}")
    } else {
      raw_specifier
        .split('/')
        .next()
        .unwrap_or_default()
        .split('@')
        .next()
        .unwrap_or_default()
        .to_string()
    }
  }

  fn parse_jsr_package_name_and_subpath(raw_specifier: &str) -> (String, String) {
    if raw_specifier.starts_with('@') {
      let mut it = raw_specifier.splitn(3, '/');
      let scope = it.next().unwrap_or_default();
      let name_and_rest = it.next().unwrap_or_default();
      let base_name = name_and_rest.split('@').next().unwrap_or(name_and_rest);
      let package_name = format!("{scope}/{base_name}");
      let subpath = it.next().unwrap_or_default().to_string();
      (package_name, subpath)
    } else {
      let mut it = raw_specifier.splitn(2, '/');
      let package_name = it
        .next()
        .unwrap_or_default()
        .split('@')
        .next()
        .unwrap_or_default()
        .to_string();
      let subpath = it.next().unwrap_or_default().to_string();
      (package_name, subpath)
    }
  }

  fn load_shim_module(
    module_specifier: &deno_core::ModuleSpecifier,
  ) -> Result<ModuleSource, ModuleLoaderError> {
    let spec = module_specifier.as_str();
    if let Some(raw) = spec.strip_prefix("npm:") {
      match Self::parse_npm_package_name(raw).as_str() {
        "nanoid" | "date-fns" | "lodash-es" | "zod" | "stream-chat" => {}
        pkg => {
          return Err(JsErrorBox::generic(format!(
            "unsupported npm package in direct mode: {pkg}"
          )));
        }
      }
    } else if let Some(raw) = spec.strip_prefix("jsr:") {
      let (package_name, subpath) = Self::parse_jsr_package_name_and_subpath(raw);
      match package_name.as_str() {
        "@std/dotenv" => {}
        "@std/async" => {
          if !(subpath.is_empty() || subpath == "delay") {
            return Err(JsErrorBox::generic(format!(
              "unsupported jsr subpath in direct mode: @std/async/{subpath}"
            )));
          }
        }
        pkg => {
          return Err(JsErrorBox::generic(format!(
            "unsupported jsr package in direct mode: {pkg}"
          )));
        }
      }
    } else {
      return Err(JsErrorBox::generic(format!(
        "unsupported module scheme: {}",
        module_specifier.scheme()
      )));
    }

    Ok(ModuleSource::new(
      ModuleType::JavaScript,
      ModuleSourceCode::String(SHIM_MODULES_SOURCE.to_string().into()),
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
    if specifier.starts_with("npm:") || specifier.starts_with("jsr:") {
      return deno_core::ModuleSpecifier::parse(specifier).map_err(JsErrorBox::from_err);
    }
    resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    module_specifier: &deno_core::ModuleSpecifier,
    _maybe_referrer: Option<&ModuleLoadReferrer>,
    _options: ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let result = match module_specifier.scheme() {
      "file" => Self::load_file_module(self.source_maps.clone(), module_specifier),
      "npm" | "jsr" => Self::load_shim_module(module_specifier),
      scheme => Err(JsErrorBox::generic(format!(
        "unsupported module scheme: {scheme}"
      ))),
    };

    ModuleLoadResponse::Sync(result)
  }

  fn get_source_map(&self, specifier: &str) -> Option<Cow<'_, [u8]>> {
    self
      .source_maps
      .borrow()
      .get(specifier)
      .map(|source_map| Cow::Owned(source_map.clone()))
  }
}

#[op2(fast)]
#[smi]
fn op_duplex_open(state: &mut OpState) -> Result<ResourceId, JsErrorBox> {
  let slot = state.borrow::<DuplexChannelSlot>().clone();
  let mut guard = slot
    .channels
    .lock()
    .map_err(|_| JsErrorBox::generic("failed to lock duplex channel slot"))?;
  let channels = guard
    .take()
    .ok_or_else(|| JsErrorBox::generic("duplex channel already opened"))?;
  Ok(state.resource_table.add(TokioDuplexResource {
    inbound_rx: tokio::sync::Mutex::new(channels.inbound_rx),
    outbound_tx: channels.outbound_tx,
  }))
}

#[op2]
#[string]
async fn op_duplex_read_line(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
) -> Result<String, JsErrorBox> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TokioDuplexResource>(rid)
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;
  let mut inbound_rx = resource.inbound_rx.lock().await;
  read_line(&mut inbound_rx)
    .await
    .map_err(|err| JsErrorBox::generic(err.to_string()))
}

#[op2]
#[smi]
async fn op_duplex_write_line(
  state: Rc<RefCell<OpState>>,
  #[smi] rid: ResourceId,
  #[string] line: String,
) -> Result<u32, JsErrorBox> {
  let resource = state
    .borrow()
    .resource_table
    .get::<TokioDuplexResource>(rid)
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;
  let written = line.len() as u32;
  write_line(&resource.outbound_tx, line)
    .await
    .map_err(|err| JsErrorBox::generic(err.to_string()))?;
  Ok(written)
}

deno_core::extension!(
  libmainworker_duplex_ext,
  ops = [op_duplex_open, op_duplex_read_line, op_duplex_write_line],
  options = {
    channel_slot: DuplexChannelSlot,
  },
  state = |state, options| {
    state.put(options.channel_slot);
  }
);

fn duplex_extension(channels: DuplexChannelPair) -> deno_core::Extension {
  let mut ext = libmainworker_duplex_ext::init(DuplexChannelSlot {
    channels: Arc::new(Mutex::new(Some(channels))),
  });
  ext
    .esm_files
    .to_mut()
    .push(ExtensionFileSource::new_computed(
      DUPLEX_API_SPECIFIER,
      Arc::<str>::from(DUPLEX_API_SOURCE),
    ));
  ext.esm_entry_point = Some(DUPLEX_API_SPECIFIER);
  ext
}

#[op2(fast)]
fn libmainworker_embed_set_result(state: &mut OpState, #[string] value: String) {
  let holder = state.borrow::<Arc<Mutex<EmbedResult>>>();
  if let Ok(mut slot) = holder.lock() {
    slot.result = Some(value);
  }
}

#[op2(fast)]
fn libmainworker_embed_set_exit_data(state: &mut OpState, #[string] value: String) {
  let holder = state.borrow::<Arc<Mutex<EmbedResult>>>();
  if let Ok(mut slot) = holder.lock() {
    slot.exit_data = Some(value);
  }
}

deno_core::extension!(
  libmainworker_embed_ext,
  ops = [libmainworker_embed_set_result, libmainworker_embed_set_exit_data],
  options = {
    result_holder: Arc<Mutex<EmbedResult>>,
  },
  state = |state, options| {
    state.put(options.result_holder);
  }
);

fn embed_extension(result_holder: Arc<Mutex<EmbedResult>>) -> deno_core::Extension {
  let mut ext = libmainworker_embed_ext::init(result_holder);
  ext
    .esm_files
    .to_mut()
    .push(ExtensionFileSource::new_computed(
      EMBED_RESULT_SPECIFIER,
      Arc::<str>::from(EMBED_RESULT_SOURCE),
    ));
  ext.esm_entry_point = Some(EMBED_RESULT_SPECIFIER);
  ext
}

deno_core::extension!(
  snapshot_options_extension,
  options = {
    snapshot_options: SnapshotOptions,
  },
  state = |state, options| {
    state.put::<SnapshotOptions>(options.snapshot_options);
  },
);

async fn read_line(rx: &mut mpsc::Receiver<String>) -> Result<String, AnyError> {
  rx.recv()
    .await
    .ok_or_else(|| AnyError::msg("duplex channel reached EOF"))
}

async fn write_line(tx: &mpsc::Sender<String>, line: String) -> Result<(), AnyError> {
  tx.send(line)
    .await
    .map_err(|err| AnyError::msg(format!("duplex channel send failed: {err}")))
}

async fn write_json_line(
  tx: &mpsc::Sender<String>,
  value: &serde_json::Value,
) -> Result<(), AnyError> {
  let line = serde_json::to_string(value).map_err(|err| AnyError::msg(err.to_string()))?;
  write_line(tx, line).await
}

fn execute_ts_initiated_rust_call(
  payload: &serde_json::Value,
) -> Result<serde_json::Value, AnyError> {
  let op = payload.get("op").and_then(|v| v.as_str()).unwrap_or("echo");

  match op {
    "uppercase" => {
      let text = payload
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AnyError::msg("rust_call.uppercase requires string field `text`"))?;
      Ok(serde_json::json!({
        "op": "uppercase",
        "output": text.to_uppercase(),
      }))
    }
    "sum" => {
      let values = payload
        .get("values")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AnyError::msg("rust_call.sum requires array field `values`"))?;
      let mut sum = 0.0_f64;
      for value in values {
        let number = value
          .as_f64()
          .ok_or_else(|| AnyError::msg("rust_call.sum expects numbers only"))?;
        sum += number;
      }
      Ok(serde_json::json!({
        "op": "sum",
        "output": sum,
      }))
    }
    "echo" => Ok(serde_json::json!({
      "op": "echo",
      "output": payload,
    })),
    _ => Err(AnyError::msg(format!("unsupported rust_call op: {op}"))),
  }
}

async fn rust_duplex_driver(
  rust_to_ts_tx: mpsc::Sender<String>,
  mut ts_to_rust_rx: mpsc::Receiver<String>,
) -> Result<(), AnyError> {
  let mut interval = tokio::time::interval(Duration::from_millis(300));
  interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
  interval.tick().await;

  let mut ping_seq = 0_u64;
  let mut pong_count = 0_u64;
  let mut is_ready = false;
  let mut sent_demo_message = false;
  let mut sent_shutdown = false;

  loop {
    tokio::select! {
      _ = interval.tick(), if !sent_shutdown => {
        ping_seq += 1;
        write_json_line(
          &rust_to_ts_tx,
          &serde_json::json!({
            "type": "ping",
            "seq": ping_seq,
            "from": "rust",
          }),
        )
        .await?;

        if is_ready && !sent_demo_message {
          write_json_line(
            &rust_to_ts_tx,
            &serde_json::json!({
              "type": "message",
              "id": "demo-1",
              "payload": {
                "text": "hello from rust",
                "seq": ping_seq,
              }
            }),
          )
          .await?;
          sent_demo_message = true;
        }

        if is_ready && pong_count >= 3 {
          write_json_line(
            &rust_to_ts_tx,
            &serde_json::json!({
              "type": "shutdown",
              "reason": "demo_completed",
            }),
          )
          .await?;
          sent_shutdown = true;
        } else if ping_seq >= 10 {
          write_json_line(
            &rust_to_ts_tx,
            &serde_json::json!({
              "type": "shutdown",
              "reason": "timeout",
            }),
          )
          .await?;
          sent_shutdown = true;
        }
      }
      inbound = read_line(&mut ts_to_rust_rx) => {
        let inbound = inbound?;
        println!("[rust] received: {inbound}");
        let Ok(message) = serde_json::from_str::<serde_json::Value>(&inbound) else {
          continue;
        };

        match message.get("type").and_then(|v| v.as_str()) {
          Some("ready") => {
            is_ready = true;
          }
          Some("pong") => {
            pong_count += 1;
          }
          Some("message_result") => {
            if let Some(result) = message.get("result") {
              println!("[rust] message result: {result}");
            }
          }
          Some("rust_call") => {
            let id = message.get("id").cloned().unwrap_or(serde_json::Value::Null);
            let payload = message
              .get("payload")
              .cloned()
              .unwrap_or(serde_json::Value::Null);

            match execute_ts_initiated_rust_call(&payload) {
              Ok(result) => {
                write_json_line(
                  &rust_to_ts_tx,
                  &serde_json::json!({
                    "type": "rust_call_result",
                    "id": id,
                    "result": result,
                  }),
                )
                .await?;
              }
              Err(err) => {
                write_json_line(
                  &rust_to_ts_tx,
                  &serde_json::json!({
                    "type": "rust_call_error",
                    "id": id,
                    "error": err.to_string(),
                  }),
                )
                .await?;
              }
            }
          }
          Some("shutdown_ack") => {
            break;
          }
          Some("error") => {
            return Err(AnyError::msg(format!("ts message error: {message}")));
          }
          _ => {}
        }
      }
    }
  }

  Ok(())
}

fn resolve_target_specifier(arg: &str) -> Result<String, AnyError> {
  let is_url_like = arg.starts_with("file://")
    || arg.starts_with("http://")
    || arg.starts_with("https://")
    || arg.starts_with("jsr:")
    || arg.starts_with("npm:");

  if is_url_like {
    return Ok(arg.to_string());
  }

  let path = PathBuf::from(arg);
  let abs_path = if path.is_absolute() {
    path
  } else {
    let cwd = std::env::current_dir()?;
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
      .parent()
      .map(PathBuf::from)
      .unwrap_or_else(|| manifest_dir.clone());

    let mut candidates = Vec::with_capacity(5);
    candidates.push(cwd.join(&path));
    candidates.push(manifest_dir.join(&path));
    candidates.push(workspace_root.join(&path));
    candidates.push(cwd.join("embed_deno").join(&path));
    candidates.push(workspace_root.join("embed_deno").join(&path));

    if let Some(found) = candidates.iter().find(|candidate| candidate.is_file()) {
      found.clone()
    } else {
      let tried = candidates
        .into_iter()
        .map(|candidate| candidate.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
      return Err(AnyError::msg(format!(
        "target script not found for `{arg}`; looked in: {tried}"
      )));
    }
  };

  Url::from_file_path(&abs_path)
    .map(|url| url.to_string())
    .map_err(|_| {
      AnyError::msg(format!(
        "failed to convert path to file url: {}",
        abs_path.display()
      ))
    })
}

fn bootstrap_script_path() -> Result<PathBuf, AnyError> {
  let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("src")
    .join("duplex_bootstrap.ts");
  if !path.exists() {
    return Err(AnyError::msg(format!(
      "duplex bootstrap script not found: {}",
      path.display()
    )));
  }
  Ok(path)
}

#[tokio::main]
async fn main() -> Result<(), AnyError> {
  let (result_tx, result_rx) = tokio::sync::oneshot::channel::<Result<(), AnyError>>();

  std::thread::Builder::new()
    .name("libmainworker_duplex_runtime".to_string())
    .spawn(move || {
      let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
      {
        Ok(runtime) => runtime,
        Err(err) => {
          let _ = result_tx.send(Err(AnyError::msg(format!(
            "failed to create current-thread runtime: {err}"
          ))));
          return;
        }
      };

      let local = tokio::task::LocalSet::new();
      let result = local.block_on(&runtime, run_inner());
      let _ = result_tx.send(result);
    })
    .map_err(|err| AnyError::msg(format!("failed to spawn runtime thread: {err}")))?;

  result_rx
    .await
    .map_err(|err| AnyError::msg(format!("runtime thread dropped result: {err}")))?
}

async fn run_inner() -> Result<(), AnyError> {
  // Required by rustls 0.23+ when TLS-backed APIs (for example fetch) are used.
  let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var("DENO_FORCE_OP_REGISTRATION", "1");
  }

  let target_arg = std::env::args()
    .nth(1)
    .unwrap_or_else(|| "embed_deno/simple_main.ts".to_string());
  let target_specifier = resolve_target_specifier(&target_arg)?;
  let bootstrap_path = bootstrap_script_path()?;

  #[allow(clippy::undocumented_unsafe_blocks)]
  unsafe {
    std::env::set_var("LIBMAINWORKER_TARGET_SPECIFIER", &target_specifier);
  }

  println!("target script: {target_arg}");
  println!("target specifier: {target_specifier}");

  let (rust_to_ts_tx, rust_to_ts_rx) = mpsc::channel::<String>(64);
  let (ts_to_rust_tx, ts_to_rust_rx) = mpsc::channel::<String>(64);
  let embed_result = Arc::new(Mutex::new(EmbedResult::default()));
  let embed_result_for_worker = embed_result.clone();

  let root_permissions = deno_runtime::deno_permissions::PermissionsContainer::allow_all(Arc::new(
    deno_runtime::permissions::RuntimePermissionDescriptorParser::new(
      sys_traits::impls::RealSys::default(),
    ),
  ));
  let module_loader = Rc::new(DirectModuleLoader::new());

  let main_module = Url::from_file_path(&bootstrap_path)
    .map(deno_core::ModuleSpecifier::from)
    .map_err(|_| {
      AnyError::msg(format!(
        "failed to convert bootstrap path to file url: {}",
        bootstrap_path.display()
      ))
    })?;
  let services = WorkerServiceOptions::<
    DenoInNpmPackageChecker,
    NpmResolver<sys_traits::impls::RealSys>,
    sys_traits::impls::RealSys,
  > {
    deno_rt_native_addon_loader: None,
    module_loader,
    permissions: root_permissions,
    blob_store: Arc::new(deno_runtime::deno_web::BlobStore::default()),
    broadcast_channel: Default::default(),
    feature_checker: Arc::new(deno_runtime::FeatureChecker::default()),
    node_services: None,
    npm_process_state_provider: None,
    root_cert_store_provider: None,
    fetch_dns_resolver: Default::default(),
    shared_array_buffer_store: None,
    compiled_wasm_module_store: None,
    v8_code_cache: None,
    bundle_provider: None,
    fs: Arc::new(deno_runtime::deno_fs::RealFs),
  };
  let options = WorkerOptions {
    startup_snapshot: deno_snapshots::CLI_SNAPSHOT,
    bootstrap: BootstrapOptions {
      mode: WorkerExecutionMode::Run,
      enable_testing_features: true,
      ..Default::default()
    },
    extensions: vec![
      snapshot_options_extension::init(SnapshotOptions::default()),
      duplex_extension(DuplexChannelPair {
        inbound_rx: rust_to_ts_rx,
        outbound_tx: ts_to_rust_tx,
      }),
      embed_extension(embed_result_for_worker),
    ],
    ..Default::default()
  };
  let mut worker = MainWorker::bootstrap_from_options(&main_module.clone(), services, options);

  println!("mainworker created with direct MainWorker bootstrap + duplex extension");
  let worker_main_module = main_module.clone();
  let mut worker_future = std::pin::pin!(async move {
    let module_id = worker.preload_main_module(&worker_main_module).await?;
    worker.evaluate_module(module_id).await?;
    worker.run_event_loop(false).await?;
    Ok::<(), AnyError>(())
  });
  let mut driver_future = std::pin::pin!(rust_duplex_driver(rust_to_ts_tx, ts_to_rust_rx));
  let mut worker_completed = false;
  let mut driver_completed = false;

  loop {
    tokio::select! {
      worker_result = &mut worker_future, if !worker_completed => {
        worker_result?;
        worker_completed = true;
        if driver_completed {
          break;
        }
      }
      driver_result = &mut driver_future, if !driver_completed => {
        driver_result?;
        driver_completed = true;
        if worker_completed {
          break;
        }
      }
    }
  }

  if !worker_completed {
    return Err(AnyError::msg("worker did not complete"));
  }
  if !driver_completed {
    return Err(AnyError::msg("rust duplex driver did not complete"));
  }

  println!("worker completed (direct MainWorker mode, no CLI exit code)");
  println!("rust <-> ts channel communication completed");

  if let Ok(mut guard) = embed_result.lock() {
    if let Some(json) = guard.exit_data.take() {
      println!("EMBED_DENO_EXIT_DATA={json}");
    }
    if let Some(json) = guard.result.take() {
      println!("EMBED_DENO_RESULT={json}");
    }
  }

  Ok(())
}
