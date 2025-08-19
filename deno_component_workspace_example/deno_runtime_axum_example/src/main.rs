use std::{rc::Rc, sync::Arc};

mod module_loader;

use anyhow::Result;
use axum::{
  Router,
  // extract::Request,
  http::{HeaderMap, StatusCode},
  routing::get,
};
use deno_resolver::npm::{ByonmInNpmPackageChecker, ByonmNpmResolver};
use deno_runtime::{
  BootstrapOptions, WorkerExecutionMode,
  deno_broadcast_channel::InMemoryBroadcastChannel,
  deno_core::{self, FastString, FsModuleLoader, ModuleSpecifier},
  deno_fs::RealFs,
  deno_io::Stdio,
  deno_permissions::{Permissions, PermissionsContainer},
  deno_tls::rustls::crypto::{CryptoProvider, ring},
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use module_loader::NpmAwareModuleLoader;
use sys_traits::impls::RealSys;
use tokio::sync::mpsc;

// Simple approach - focus on BYONM (node_modules) resolution

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
  NodeHttpsTest {
    response_tx: tokio::sync::oneshot::Sender<Result<String, String>>,
  },
}

async fn deno_runtime_task(mut rx: mpsc::Receiver<DenoCommand>) {
  // Install the default crypto provider for rustls (required for HTTPS)
  if CryptoProvider::install_default(ring::default_provider()).is_err() {
    eprintln!("Warning: Failed to install default crypto provider - may already be set");
  }

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
          DenoCommand::NodeHttpsTest { response_tx } => {
            let result = execute_node_https_test().await;
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
  let sys = RealSys;
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(sys));
  let permissions = PermissionsContainer::new(permission_desc_parser, Permissions::allow_all());

  // Set up stdio
  let stdio = Stdio {
    stdin: deno_runtime::deno_io::StdioPipe::inherit(),
    stdout: deno_runtime::deno_io::StdioPipe::inherit(),
    stderr: deno_runtime::deno_io::StdioPipe::inherit(),
  };

  // Create worker options with proper bootstrap
  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      deno_version: "0.1.0".to_string(),
      args: vec![],
      cpu_count: std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1),
      log_level: deno_runtime::WorkerLogLevel::Info,
      enable_op_summary_metrics: false,
      enable_testing_features: true,
      locale: "en-US".to_string(),
      location: None,
      color_level: deno_terminal::colors::ColorLevel::Ansi256,
      unstable_features: vec![],
      user_agent: "deno_runtime_axum_example".to_string(),
      inspect: false,
      is_standalone: false,
      has_node_modules_dir: false,
      argv0: None,
      node_debug: None,
      node_ipc_fd: None,
      mode: WorkerExecutionMode::Run,
      no_legacy_abort: false,
      serve_port: None,
      serve_host: None,
      auto_serve: false,
      otel_config: Default::default(),
      close_on_idle: false,
    },
    extensions: vec![],
    startup_snapshot: None,
    create_params: None,
    unsafely_ignore_certificate_errors: None,
    seed: None,
    create_web_worker_cb: Arc::new(|_| {
      unreachable!("Web workers are not supported in this example")
    }),
    format_js_error_fn: None,
    maybe_inspector_server: None,
    should_break_on_first_statement: false,
    should_wait_for_inspector_session: false,
    strace_ops: None,
    cache_storage_dir: None,
    origin_storage_dir: None,
    stdio,
    skip_op_registration: false,
    enable_raw_imports: false,
    enable_stack_trace_arg_in_ops: false,
    unconfigured_runtime: None,
  };

  // Create service options
  let services =
    WorkerServiceOptions::<ByonmInNpmPackageChecker, ByonmNpmResolver<RealSys>, RealSys> {
      deno_rt_native_addon_loader: None,
      module_loader: Rc::new(FsModuleLoader),
      permissions,
      blob_store: Default::default(),
      broadcast_channel: InMemoryBroadcastChannel::default(),
      feature_checker: Default::default(),
      node_services: Default::default(),
      npm_process_state_provider: None,
      root_cert_store_provider: None,
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: None,
      compiled_wasm_module_store: None,
      v8_code_cache: None,
      fs,
    };

  // Create MainWorker with proper bootstrap
  let mut worker = MainWorker::bootstrap_from_options(&main_module, services, options);

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
  let sys = RealSys;
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(sys));
  let permissions = PermissionsContainer::new(permission_desc_parser, Permissions::allow_all());

  // Set up stdio
  let stdio = Stdio {
    stdin: deno_runtime::deno_io::StdioPipe::inherit(),
    stdout: deno_runtime::deno_io::StdioPipe::inherit(),
    stderr: deno_runtime::deno_io::StdioPipe::inherit(),
  };

  // Create worker options with proper bootstrap
  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      deno_version: "0.1.0".to_string(),
      args: vec![],
      cpu_count: std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1),
      log_level: deno_runtime::WorkerLogLevel::Info,
      enable_op_summary_metrics: false,
      enable_testing_features: true,
      locale: "en-US".to_string(),
      location: None,
      color_level: deno_terminal::colors::ColorLevel::Ansi256,
      unstable_features: vec![],
      user_agent: "deno_runtime_axum_example".to_string(),
      inspect: false,
      is_standalone: false,
      has_node_modules_dir: true, // Enable node_modules for stream-token
      argv0: None,
      node_debug: None,
      node_ipc_fd: None,
      mode: WorkerExecutionMode::Run,
      no_legacy_abort: false,
      serve_port: None,
      serve_host: None,
      auto_serve: false,
      otel_config: Default::default(),
      close_on_idle: false,
    },
    extensions: vec![],
    startup_snapshot: None,
    create_params: None,
    unsafely_ignore_certificate_errors: None,
    seed: None,
    create_web_worker_cb: Arc::new(|_| {
      unreachable!("Web workers are not supported in this example")
    }),
    format_js_error_fn: None,
    maybe_inspector_server: None,
    should_break_on_first_statement: false,
    should_wait_for_inspector_session: false,
    strace_ops: None,
    cache_storage_dir: None,
    origin_storage_dir: None,
    stdio,
    skip_op_registration: false,
    enable_raw_imports: false,
    enable_stack_trace_arg_in_ops: false,
    unconfigured_runtime: None,
  };

  // Create service options
  let services =
    WorkerServiceOptions::<ByonmInNpmPackageChecker, ByonmNpmResolver<RealSys>, RealSys> {
      deno_rt_native_addon_loader: None,
      module_loader: Rc::new(NpmAwareModuleLoader::new()),
      permissions,
      blob_store: Default::default(),
      broadcast_channel: InMemoryBroadcastChannel::default(),
      feature_checker: Default::default(),
      node_services: Default::default(),
      npm_process_state_provider: None,
      root_cert_store_provider: None,
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: None,
      compiled_wasm_module_store: None,
      v8_code_cache: None,
      fs,
    };

  // Create MainWorker with proper bootstrap
  let mut worker = MainWorker::bootstrap_from_options(&main_module, services, options);

  // Set up environment variables for the Deno runtime
  let stream_api_key =
    std::env::var("STREAM_API_KEY").unwrap_or_else(|_| "mock_stream_api_key".to_string());
  let stream_api_secret =
    std::env::var("STREAM_API_SECRET").unwrap_or_else(|_| "mock_stream_api_secret".to_string());

  let env_setup = format!(
    r#"
    globalThis.STREAM_API_KEY = "{}";
    globalThis.STREAM_API_SECRET = "{}";
    
    // Polyfill for Node.js https.Agent to fix stream-chat compatibility
    if (!globalThis.https) {{
      globalThis.https = {{}};
    }}
    if (!globalThis.https.Agent) {{
      // Simple Agent polyfill that stream-chat needs
      globalThis.https.Agent = class Agent {{
        constructor(options = {{}}) {{
          this.options = options;
        }}
      }};
    }}
    
    // Also set up http.Agent if needed
    if (!globalThis.http) {{
      globalThis.http = {{}};
    }}
    if (!globalThis.http.Agent) {{
      globalThis.http.Agent = globalThis.https.Agent;
    }}
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

async fn execute_node_https_test() -> Result<String, String> {
  let current_dir = std::env::current_dir().unwrap();
  let ts_file_path = current_dir.join("test_https_simple.ts");
  let file_url = format!("file://{}", ts_file_path.to_string_lossy());
  let main_module = ModuleSpecifier::parse(&file_url).unwrap();

  let fs = Arc::new(RealFs);
  let sys = RealSys;
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(sys));
  let permissions = PermissionsContainer::new(permission_desc_parser, Permissions::allow_all());

  // Set up stdio
  let stdio = Stdio {
    stdin: deno_runtime::deno_io::StdioPipe::inherit(),
    stdout: deno_runtime::deno_io::StdioPipe::inherit(),
    stderr: deno_runtime::deno_io::StdioPipe::inherit(),
  };

  // Create worker options with proper bootstrap
  let options = WorkerOptions {
    bootstrap: BootstrapOptions {
      deno_version: "0.1.0".to_string(),
      args: vec![],
      cpu_count: std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(1),
      log_level: deno_runtime::WorkerLogLevel::Info,
      enable_op_summary_metrics: false,
      enable_testing_features: true,
      locale: "en-US".to_string(),
      location: None,
      color_level: deno_terminal::colors::ColorLevel::Ansi256,
      unstable_features: vec![],
      user_agent: "deno_runtime_axum_example".to_string(),
      inspect: false,
      is_standalone: false,
      has_node_modules_dir: true,
      argv0: None,
      node_debug: None,
      node_ipc_fd: None,
      mode: WorkerExecutionMode::Run,
      no_legacy_abort: false,
      serve_port: None,
      serve_host: None,
      auto_serve: false,
      otel_config: Default::default(),
      close_on_idle: false,
    },
    extensions: vec![],
    startup_snapshot: None,
    create_params: None,
    unsafely_ignore_certificate_errors: None,
    seed: None,
    create_web_worker_cb: Arc::new(|_| {
      unreachable!("Web workers are not supported in this example")
    }),
    format_js_error_fn: None,
    maybe_inspector_server: None,
    should_break_on_first_statement: false,
    should_wait_for_inspector_session: false,
    strace_ops: None,
    cache_storage_dir: None,
    origin_storage_dir: None,
    stdio,
    skip_op_registration: false,
    enable_raw_imports: false,
    enable_stack_trace_arg_in_ops: false,
    unconfigured_runtime: None,
  };

  // Create service options with FsModuleLoader for better Node.js compatibility
  let services =
    WorkerServiceOptions::<ByonmInNpmPackageChecker, ByonmNpmResolver<RealSys>, RealSys> {
      deno_rt_native_addon_loader: None,
      module_loader: Rc::new(FsModuleLoader),
      permissions,
      blob_store: Default::default(),
      broadcast_channel: InMemoryBroadcastChannel::default(),
      feature_checker: Default::default(),
      node_services: Default::default(),
      npm_process_state_provider: None,
      root_cert_store_provider: None,
      fetch_dns_resolver: Default::default(),
      shared_array_buffer_store: None,
      compiled_wasm_module_store: None,
      v8_code_cache: None,
      fs,
    };

  // Create MainWorker with proper bootstrap
  let mut worker = MainWorker::bootstrap_from_options(&main_module, services, options);

  // Load and evaluate the TypeScript module
  let module_id = worker
    .preload_main_module(&main_module)
    .await
    .map_err(|e| format!("Node HTTPS test module preload error: {}", e))?;

  let _result = worker
    .evaluate_module(module_id)
    .await
    .map_err(|e| format!("Node HTTPS test module evaluation error: {}", e))?;

  // Run the event loop to let the HTTPS request complete
  worker
    .run_event_loop(false)
    .await
    .map_err(|e| format!("Node HTTPS test event loop error: {}", e))?;

  // Get the test result from the global variable
  let get_result_script = r#"
    if (globalThis.nodeHttpsTestResult) {
      JSON.stringify(globalThis.nodeHttpsTestResult, null, 2)
    } else {
      "No test result found"
    }
  "#
  .to_string();

  let result = worker
    .execute_script(
      "<get-node-test-result>",
      FastString::from(get_result_script),
    )
    .map_err(|e| format!("Get result error: {}", e))?;

  let scope = &mut worker.js_runtime.handle_scope();
  let local_result = deno_core::v8::Local::new(scope, result);
  let result_str = local_result.to_rust_string_lossy(scope);

  Ok(format!(
    "Node.js HTTP/HTTPS test completed!\n{}",
    result_str
  ))
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

#[axum::debug_handler]
pub async fn node_https_test_handler(
  axum::extract::State(tx): axum::extract::State<mpsc::Sender<DenoCommand>>,
) -> Result<String, (StatusCode, String)> {
  let (response_tx, response_rx) = tokio::sync::oneshot::channel();

  let command = DenoCommand::NodeHttpsTest { response_tx };

  tx.send(command).await.map_err(|_| {
    (
      StatusCode::INTERNAL_SERVER_ERROR,
      "Failed to send node https test command".to_string(),
    )
  })?;

  match response_rx.await {
    Ok(Ok(result)) => Ok(result),
    Ok(Err(e)) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    Err(_) => Err((
      StatusCode::INTERNAL_SERVER_ERROR,
      "Failed to receive node https test response".to_string(),
    )),
  }
}

#[tokio::main]
async fn main() {
  // Load environment variables from .env file
  dotenvy::dotenv().ok();

  // Create channel for communication
  let (tx, rx) = mpsc::channel(100);

  // Spawn Deno runtime task in a separate thread to handle LocalSet
  std::thread::spawn(move || {
    let runtime = tokio::runtime::Builder::new_current_thread()
      .enable_all()
      .build()
      .unwrap();

    runtime.block_on(async move {
      deno_runtime_task(rx).await;
    });
  });

  // Create router with state
  let router = Router::new()
    .route("/test", get(handler))
    .route("/stream-token", get(stream_token_handler))
    .route("/node-https-test", get(node_https_test_handler))
    .with_state(tx);

  let addr = format!("0.0.0.0:{}", 7777);

  let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
  println!("Server listening on {}", addr);

  axum::serve(listener, router).await.unwrap();
}
