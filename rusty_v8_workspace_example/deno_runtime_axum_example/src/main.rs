use std::{rc::Rc, sync::Arc};

use axum::{
  Router,
  // extract::Request,
  http::{HeaderMap, StatusCode},
  routing::get,
};
use deno_error::JsErrorBox;
use deno_resolver::npm::{ByonmInNpmPackageChecker, ByonmNpmResolver};
use deno_runtime::{
  deno_core::{
    self, FastString, FsModuleLoader, ModuleLoader, ModuleSource, ModuleSourceCode,
    ModuleSpecifier, ModuleType, RequestedModuleType, ResolutionKind,
  },
  deno_fs::RealFs,
  deno_permissions::PermissionsContainer,
  ops::bootstrap::SnapshotOptions,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use sys_traits::impls::RealSys;
use tokio::sync::mpsc;

// Custom module loader that handles npm imports
struct CustomModuleLoader {
  fs_loader: FsModuleLoader,
}

impl CustomModuleLoader {
  fn new() -> Self {
    Self {
      fs_loader: FsModuleLoader,
    }
  }
}

impl ModuleLoader for CustomModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: ResolutionKind,
  ) -> Result<ModuleSpecifier, JsErrorBox> {
    // Handle npm imports by mocking them
    if specifier.starts_with("npm:") {
      // For npm:stream-chat, we'll mock it with a local implementation
      if specifier == "npm:stream-chat" {
        // Create a mock module specifier with proper JavaScript
        let js_code = "export const StreamChat = { getInstance: function(apiKey, apiSecret) { \
                       return { createToken: function(userId) { return 'mock_token_' + userId + \
                       '_' + Date.now(); } }; } };";
        let data_url = format!("data:text/javascript,{}", urlencoding::encode(js_code));
        return Ok(
          ModuleSpecifier::parse(&data_url)
            .map_err(|e| JsErrorBox::generic(format!("Parse error: {}", e)))?,
        );
      }
    }

    // For other modules, use the default file system loader
    self.fs_loader.resolve(specifier, referrer, _kind)
  }

  fn load(
    &self,
    module_specifier: &ModuleSpecifier,
    _maybe_referrer: Option<&ModuleSpecifier>,
    _is_dyn_import: bool,
    _requested_module_type: RequestedModuleType,
  ) -> deno_core::ModuleLoadResponse {
    // Handle data URLs (our mocked npm modules)
    if module_specifier.scheme() == "data" {
      let content = module_specifier.as_str().split(',').nth(1).unwrap_or("");
      // Decode URL-encoded content
      let decoded_content = urlencoding::decode(content).unwrap_or_else(|_| content.into());
      let source = ModuleSource::new(
        ModuleType::JavaScript,
        ModuleSourceCode::String(FastString::from(decoded_content.to_string())),
        module_specifier,
        None,
      );
      return deno_core::ModuleLoadResponse::Sync(Ok(source));
    }

    // For other modules, use the default file system loader
    self.fs_loader.load(
      module_specifier,
      _maybe_referrer,
      _is_dyn_import,
      _requested_module_type,
    )
  }
}

// Extension to provide SnapshotOptions
deno_core::extension!(
    snapshot_options_extension,
    options = {
        snapshot_options: SnapshotOptions,
    },
    state = |state, options| {
        state.put::<SnapshotOptions>(options.snapshot_options);
    },
);

#[derive(Debug)]
pub enum DenoCommand {
  ExecuteScript {
    script: String,
    response_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
  },
  StreamToken {
    auth_header: String,
    response_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
  },
}

async fn deno_runtime_task(mut rx: mpsc::Receiver<DenoCommand>) {
  // Use LocalSet to run !Send futures
  let local = tokio::task::LocalSet::new();

  local
    .run_until(async move {
      while let Some(command) = rx.recv().await {
        match command {
          DenoCommand::ExecuteScript {
            script,
            response_tx,
          } => {
            let result = execute_script(script).await;
            let _ = response_tx.send(result);
          }
          DenoCommand::StreamToken {
            auth_header,
            response_tx,
          } => {
            let result = execute_stream_token(auth_header).await;
            let _ = response_tx.send(result);
          }
        }
      }
    })
    .await;
}

async fn execute_script(script: String) -> Result<String, String> {
  // Create a simple module to execute
  let main_module = ModuleSpecifier::parse("file:///test.js").unwrap();

  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(RealSys));
  let snapshot_options = SnapshotOptions::default();

  // Create MainWorker instead of JsRuntime directly
  let mut worker = MainWorker::bootstrap_from_options(
    &main_module,
    WorkerServiceOptions::<ByonmInNpmPackageChecker, ByonmNpmResolver<RealSys>, RealSys> {
      deno_rt_native_addon_loader: Default::default(),
      module_loader: Rc::new(FsModuleLoader),
      permissions: PermissionsContainer::allow_all(permission_desc_parser),
      blob_store: Default::default(),
      broadcast_channel: Default::default(),
      feature_checker: Default::default(),
      node_services: None,
      npm_process_state_provider: Default::default(),
      root_cert_store_provider: Default::default(),
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      v8_code_cache: Default::default(),
      fs,
    },
    WorkerOptions {
      extensions: vec![snapshot_options_extension::init(snapshot_options)],
      ..Default::default()
    },
  );

  // Execute the provided script and capture the result
  let result = worker
    .execute_script("<script>", FastString::from(script))
    .map_err(|e| format!("Execution error: {}", e))?;

  worker
    .run_event_loop(false)
    .await
    .map_err(|e| format!("Event loop error: {}", e))?;

  // Convert the V8 value to a string representation
  let scope = &mut worker.js_runtime.handle_scope();
  let local_result = deno_core::v8::Local::new(scope, result);
  let result_str = local_result.to_rust_string_lossy(scope);

  Ok(result_str)
}

async fn execute_stream_token(auth_header: String) -> Result<String, String> {
  let current_dir = std::env::current_dir().unwrap();
  let ts_file_path = current_dir.join("stream-token.ts");
  let file_url = format!("file://{}", ts_file_path.to_string_lossy());
  let main_module = ModuleSpecifier::parse(&file_url).unwrap();

  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(RealSys));
  let snapshot_options = SnapshotOptions::default();

  let mut worker = MainWorker::bootstrap_from_options(
    &main_module,
    WorkerServiceOptions::<ByonmInNpmPackageChecker, ByonmNpmResolver<RealSys>, RealSys> {
      deno_rt_native_addon_loader: Default::default(),
      module_loader: Rc::new(CustomModuleLoader::new()),
      permissions: PermissionsContainer::allow_all(permission_desc_parser),
      blob_store: Default::default(),
      broadcast_channel: Default::default(),
      feature_checker: Default::default(),
      node_services: None,
      npm_process_state_provider: Default::default(),
      root_cert_store_provider: Default::default(),
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: Default::default(),
      compiled_wasm_module_store: Default::default(),
      v8_code_cache: Default::default(),
      fs,
    },
    WorkerOptions {
      extensions: vec![snapshot_options_extension::init(snapshot_options)],
      ..Default::default()
    },
  );

  // Set up environment variables for the Deno runtime
  let stream_api_key =
    std::env::var("STREAM_API_KEY").unwrap_or_else(|_| "mock_stream_api_key".to_string());
  let stream_api_secret =
    std::env::var("STREAM_API_SECRET").unwrap_or_else(|_| "mock_stream_api_secret".to_string());

  let env_setup = format!(
    r#"
    globalThis.STREAM_API_KEY = "{}";
    globalThis.STREAM_API_SECRET = "{}";
    "#,
    stream_api_key, stream_api_secret
  );

  // Set up environment variables first
  worker
    .execute_script("<env-setup>", FastString::from(env_setup))
    .map_err(|e| format!("Environment setup error: {}", e))?;

  // Load and evaluate the TypeScript module
  let module_id = worker
    .preload_main_module(&main_module)
    .await
    .map_err(|e| format!("Stream token module preload error: {}", e))?;

  let _result = worker
    .evaluate_module(module_id)
    .await
    .map_err(|e| format!("Stream token module evaluation error: {}", e))?;

  worker
    .run_event_loop(false)
    .await
    .map_err(|e| format!("Stream token event loop error: {}", e))?;

  // Call the async function
  let call_script = format!("generateStreamTokenSync('{}')", auth_header);
  let _result = worker
    .execute_script("<stream-token-call>", FastString::from(call_script))
    .map_err(|e| format!("Stream token function call error: {}", e))?;

  // Run event loop to complete the async operation
  worker
    .run_event_loop(false)
    .await
    .map_err(|e| format!("Stream token final event loop error: {}", e))?;

  // Get the result from the global variable
  let get_result_script = "globalThis.streamTokenError ? `ERROR: ${globalThis.streamTokenError}` \
                           : globalThis.streamTokenResult"
    .to_string();
  let result = worker
    .execute_script("<get-result>", FastString::from(get_result_script))
    .map_err(|e| format!("Get result error: {}", e))?;

  let scope = &mut worker.js_runtime.handle_scope();
  let local_result = deno_core::v8::Local::new(scope, result);
  let result_str = local_result.to_rust_string_lossy(scope);

  // Check for errors
  if result_str.starts_with("ERROR: ") {
    return Err(result_str);
  }

  Ok(result_str)
}

#[axum::debug_handler]
pub async fn handler(
  axum::extract::State(tx): axum::extract::State<mpsc::Sender<DenoCommand>>,
) -> Result<String, (StatusCode, String)> {
  let (response_tx, response_rx) = tokio::sync::oneshot::channel();

  // Generate random math expression
  let a = {
    use rand::Rng;
    let mut rng = rand::rng();
    rng.random_range(1 ..= 100)
  };
  let b = {
    use rand::Rng;
    let mut rng = rand::rng();
    rng.random_range(1 ..= 100)
  };
  let ops = ["+", "-", "*"];
  let op = {
    use rand::Rng;
    let mut rng = rand::rng();
    ops[rng.random_range(0 .. ops.len())]
  };

  let command = DenoCommand::ExecuteScript {
    script: format!("{} {} {}", a, op, b),
    response_tx,
  };

  tx.send(command).await.map_err(|_| {
    (
      StatusCode::INTERNAL_SERVER_ERROR,
      "Failed to send command".to_string(),
    )
  })?;

  match response_rx.await {
    Ok(Ok(result)) => Ok(result),
    Ok(Err(e)) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    Err(_) => Err((
      StatusCode::INTERNAL_SERVER_ERROR,
      "Failed to receive response".to_string(),
    )),
  }
}

#[axum::debug_handler]
pub async fn stream_token_handler(
  axum::extract::State(tx): axum::extract::State<mpsc::Sender<DenoCommand>>,
  headers: HeaderMap,
) -> Result<String, (StatusCode, String)> {
  let (response_tx, response_rx) = tokio::sync::oneshot::channel();

  // Extract authorization header
  let auth_header = headers
    .get("authorization")
    .and_then(|h| h.to_str().ok())
    .unwrap_or("Bearer mock_token_123")
    .to_string();

  let command = DenoCommand::StreamToken {
    auth_header,
    response_tx,
  };

  tx.send(command).await.map_err(|_| {
    (
      StatusCode::INTERNAL_SERVER_ERROR,
      "Failed to send stream token command".to_string(),
    )
  })?;

  match response_rx.await {
    Ok(Ok(result)) => Ok(result),
    Ok(Err(e)) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    Err(_) => Err((
      StatusCode::INTERNAL_SERVER_ERROR,
      "Failed to receive stream token response".to_string(),
    )),
  }
}

#[tokio::main]
async fn main() {
  // Load environment variables from .env file
  dotenvy::dotenv().ok();

  // Create channel for communication
  let (tx, rx) = mpsc::channel(100);

  // Spawn Deno runtime task with LocalSet
  std::thread::spawn(move || {
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap();

    runtime.block_on(deno_runtime_task(rx));
  });

  // Create router with state
  let router = Router::new()
    .route("/test", get(handler))
    .route("/stream-token", get(stream_token_handler))
    .with_state(tx);

  let addr = format!("0.0.0.0:{}", 7777);

  let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
  println!("Server listening on {}", addr);

  axum::serve(listener, router).await.unwrap();
}
