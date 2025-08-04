use std::{rc::Rc, sync::Arc};

use axum::{
  Router,
  // extract::Request,
  http::{HeaderMap, StatusCode},
  routing::get,
};
use deno_resolver::npm::{ByonmInNpmPackageChecker, ByonmNpmResolver};
use deno_runtime::{
  deno_core::{self, FastString, FsModuleLoader, ModuleSpecifier},
  deno_fs::RealFs,
  deno_permissions::PermissionsContainer,
  ops::bootstrap::SnapshotOptions,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use sys_traits::impls::RealSys;
use tokio::sync::mpsc;

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
  let main_module = ModuleSpecifier::parse("file:///stream-token.ts").unwrap();

  let fs = Arc::new(RealFs);
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(RealSys));
  let snapshot_options = SnapshotOptions::default();

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

  // JavaScript code for stream token generation (removed TypeScript types)
  let js_code = format!(
    r#"
// Simplified stream token generation (mock implementation)
const authHeader = "{}";

// Mock implementation - in real scenario would use Supabase and Stream Chat
function generateStreamToken(authHeader) {{
  if (!authHeader || !authHeader.startsWith('Bearer ')) {{
    throw new Error('Invalid authorization header');
  }}

  const token = authHeader.replace('Bearer ', '');
  const userId = 'user_' + Math.random().toString(36).substr(2, 9);

  // Mock token generation
  const streamToken = btoa(JSON.stringify({{
    user_id: userId,
    token: token,
    expires_at: Date.now() + 3600000 // 1 hour
  }}));

  return JSON.stringify({{
    token: streamToken,
    user_id: userId
  }});
}}

generateStreamToken(authHeader);
"#,
    auth_header
  );

  let result = worker
    .execute_script("<stream-token>", FastString::from(js_code))
    .map_err(|e| format!("Stream token execution error: {}", e))?;

  worker
    .run_event_loop(false)
    .await
    .map_err(|e| format!("Stream token event loop error: {}", e))?;

  let scope = &mut worker.js_runtime.handle_scope();
  let local_result = deno_core::v8::Local::new(scope, result);
  let result_str = local_result.to_rust_string_lossy(scope);

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
