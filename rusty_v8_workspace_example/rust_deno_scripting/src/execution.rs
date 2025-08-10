use std::{env, rc::Rc, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use deno_core::{ModuleId, PollEventLoopOptions, error::AnyError, v8};
use deno_resolver::npm::{
  ByonmInNpmPackageChecker, ByonmNpmResolver, ByonmNpmResolverCreateOptions,
};
use deno_runtime::{
  BootstrapOptions,
  deno_fs::RealFs,
  deno_node,
  deno_permissions::PermissionsContainer,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use node_resolver::{DenoIsBuiltInNodeModuleChecker, PackageJsonResolver};
use tokio::sync::RwLock;

use crate::{extension::HostState, loader::TypescriptModuleLoader};

// Create a simple node require loader
struct SimpleNodeRequireLoader;
impl deno_node::NodeRequireLoader for SimpleNodeRequireLoader {
  fn ensure_read_permission<'a>(
    &self,
    _permissions: &mut dyn deno_node::NodePermissions,
    path: std::borrow::Cow<'a, std::path::Path>,
  ) -> Result<std::borrow::Cow<'a, std::path::Path>, deno_error::JsErrorBox> {
    Ok(path)
  }

  fn load_text_file_lossy(
    &self,
    path: &std::path::Path,
  ) -> Result<deno_core::FastString, deno_error::JsErrorBox> {
    let content = std::fs::read_to_string(path)
      .map_err(|e| deno_error::JsErrorBox::new("Error", e.to_string()))?;
    Ok(deno_core::FastString::from(content))
  }

  fn is_maybe_cjs(
    &self,
    _url: &deno_core::url::Url,
  ) -> Result<bool, node_resolver::errors::ClosestPkgJsonError> {
    Ok(false)
  }
}

pub async fn run_js(
  file_path: &str,
  fn_name: &str,
  host_state: Arc<RwLock<HostState>>,
) -> Result<(), AnyError> {
  // Convert file path to module specifier (following Mako pattern)
  let main_module = deno_core::resolve_path(file_path, &env::current_dir()?)?;

  // Create module loader
  let module_loader = Rc::new(TypescriptModuleLoader::default());

  // Set up Node.js services for Node.js compatibility
  let fs = Arc::new(RealFs);
  let pkg_json_resolver = Arc::new(PackageJsonResolver::<sys_traits::impls::RealSys>::new(
    sys_traits::impls::RealSys,
    None,
  ));

  let node_require_loader = Rc::new(SimpleNodeRequireLoader);

  let npm_resolver = ByonmNpmResolver::new(ByonmNpmResolverCreateOptions {
    sys: node_resolver::cache::NodeResolutionSys::new(sys_traits::impls::RealSys, None),
    pkg_json_resolver: pkg_json_resolver.clone(),
    root_node_modules_dir: None,
  });

  let in_npm_pkg_checker = ByonmInNpmPackageChecker;

  let node_resolver = Arc::new(deno_node::NodeResolver::new(
    in_npm_pkg_checker,
    DenoIsBuiltInNodeModuleChecker,
    npm_resolver,
    pkg_json_resolver.clone(),
    node_resolver::cache::NodeResolutionSys::new(sys_traits::impls::RealSys, None),
    Default::default(),
  ));

  let node_services = deno_node::NodeExtInitServices {
    node_resolver: node_resolver.clone(),
    node_require_loader: node_require_loader.clone(),
    pkg_json_resolver: pkg_json_resolver.clone(),
    sys: sys_traits::impls::RealSys,
  };

  // Set up service options using defaults (following Mako pattern)
  let services = WorkerServiceOptions::<
    ByonmInNpmPackageChecker,
    ByonmNpmResolver<sys_traits::impls::RealSys>,
    sys_traits::impls::RealSys,
  > {
    module_loader,
    permissions: PermissionsContainer::allow_all(Arc::new(
      deno_runtime::permissions::RuntimePermissionDescriptorParser::new(sys_traits::impls::RealSys),
    )),
    blob_store: Default::default(),
    broadcast_channel: Default::default(),
    compiled_wasm_module_store: Default::default(),
    feature_checker: Default::default(),
    fs: fs.clone(),
    node_services: Some(node_services),
    npm_process_state_provider: Default::default(),
    root_cert_store_provider: Default::default(),
    shared_array_buffer_store: Default::default(),
    v8_code_cache: Default::default(),
    deno_rt_native_addon_loader: Default::default(),
    fetch_dns_resolver: Default::default(),
  };

  // Set up bootstrap options
  let bootstrap_options = BootstrapOptions::default();

  // Set up worker options with our custom extensions
  let options = WorkerOptions {
    bootstrap: bootstrap_options,
    extensions: vec![
      // Add snapshot extension for SnapshotOptions
      deno_core::Extension {
        name: "snapshot",
        op_state_fn: Some(Box::new(|state| {
          state.put(deno_runtime::ops::bootstrap::SnapshotOptions::default());
        })),
        ..Default::default()
      },
      // Add our custom extension for host state
      deno_core::Extension {
        name: "host_state",
        op_state_fn: Some(Box::new(move |state| {
          state.put(host_state.clone());
        })),
        ..Default::default()
      },
    ],
    ..Default::default()
  };

  // Create worker using bootstrap_from_options (this handles all default extensions)
  let mut worker = MainWorker::bootstrap_from_options(&main_module, services, options);

  // Execute the main module
  let module_id = worker.preload_main_module(&main_module).await?;
  worker.evaluate_module(module_id).await?;
  worker.run_event_loop(false).await?;

  // After the script has been loaded and evaluated, we can access its exports.
  let global = get_export_function_global(&mut worker, module_id, fn_name)?;

  println!("Calling function: {}", fn_name);
  let call = worker.js_runtime.call(&global);

  // Spawn HTTP client task after starting the server
  let client_handle = tokio::spawn(async move {
    // Wait a bit for the server to start
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("Making HTTP request to the server...");

    // Create a simple HTTP client request
    match make_http_request().await {
      Ok(_) => println!("HTTP request completed successfully"),
      Err(e) => eprintln!("HTTP request failed: {}", e),
    }
  });

  // Run the event loop with the server
  let server_result = worker
    .js_runtime
    .with_event_loop_promise(call, PollEventLoopOptions::default())
    .await;

  // Wait for client to finish
  let _ = client_handle.await;

  server_result?;

  Ok(())
}

fn get_export_function_global(
  worker: &mut MainWorker,
  module_id: ModuleId,
  export_name: &str,
) -> Result<v8::Global<v8::Function>> {
  // The module namespace holds all exports of the evaluated module.
  let exports_handle = worker.js_runtime.get_module_namespace(module_id)?;
  let mut scope = worker.js_runtime.handle_scope();

  let export_name_v8 =
    v8::String::new(&mut scope, export_name).context("creation of v8 string failed")?;
  let exports = exports_handle.open(&mut scope);
  let binding = exports
    .get(&mut scope, export_name_v8.into())
    .context(format!("no export named '{export_name}'"))?;

  let function: v8::Local<v8::Function> = binding.try_into()?;
  // By converting the function into a v8 global, we can decouple it from the lifetime of the
  // runtime's handle scope.
  let global = v8::Global::new(&mut scope, function);

  Ok(global)
}

async fn make_http_request() -> Result<()> {
  // Use tokio's TcpStream to make a simple HTTP request
  use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
  };

  let mut stream = TcpStream::connect("127.0.0.1:8080").await?;

  // Send HTTP request
  let request = "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
  stream.write_all(request.as_bytes()).await?;

  // Read response
  let mut response = String::new();
  stream.read_to_string(&mut response).await?;

  println!("Received response: {}", response);

  Ok(())
}
