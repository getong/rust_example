use std::{env, rc::Rc, sync::Arc};

use anyhow::{Context, Result};
use deno_core::{ModuleId, PollEventLoopOptions, error::AnyError, v8};
use deno_error::JsErrorBox;
use deno_resolver::npm::{
  ByonmInNpmPackageChecker, ByonmNpmResolver, ByonmNpmResolverCreateOptions,
};
use deno_runtime::{
  BootstrapOptions,
  deno_fs::RealFs,
  deno_node,
  deno_permissions::{Permissions, PermissionsContainer, PermissionsOptions},
  ops::bootstrap::SnapshotOptions,
  permissions::RuntimePermissionDescriptorParser,
  worker::{MainWorker, WorkerOptions, WorkerServiceOptions},
};
use node_resolver::{
  DenoIsBuiltInNodeModuleChecker, NodeResolverOptions, PackageJsonResolver,
  errors::ClosestPkgJsonError,
};
use tokio::sync::RwLock;

use crate::{
  extension::{HostState, my_extension},
  loader::TypescriptModuleLoader,
};

pub async fn run_js(
  file_path: &str,
  fn_name: &str,
  host_state: Arc<RwLock<HostState>>,
) -> Result<(), AnyError> {
  let module_loader = Rc::new(TypescriptModuleLoader::new());
  let permission_desc_parser = Arc::new(RuntimePermissionDescriptorParser::new(
    sys_traits::impls::RealSys,
  ));
  let fs = Arc::new(RealFs);
  let permissions = Permissions::from_options(
    permission_desc_parser.as_ref(),
    // Deno has a fine-grained permission system that allows configuration of
    // network access, file system access, environment variables, and more.
    // These can be further restricted to certain hostnames, IP addresses, directories, etc.
    // Further information on Deno's security mechanisms can be found at:
    //   https://docs.deno.com/runtime/fundamentals/security/
    &PermissionsOptions {
      // We only allow network access, and only to specific hostnames we want
      // our scripts to call.
      allow_net: Some(vec![
        "httpbin.org:443".to_string(),
        "httpbin.org:80".to_string(),
        "api.ipify.org:443".to_string(),
        "127.0.0.1".to_string(),
        "localhost".to_string(),
        "0.0.0.0".to_string(),
      ]),
      // Allow file system read access for Node.js modules
      allow_read: Some(vec![".".to_string()]),
      // Allow DNS resolution (might be needed for Node.js networking)
      allow_sys: Some(vec!["hostname".to_string(), "osRelease".to_string()]),
      // If set to true, scripts trying to access functions not enabled
      // by our setup will result in a command-line prompt.
      // If set to false, they are treated as if they were denied.
      prompt: false,
      ..Default::default()
    },
  )?;

  // Create node services for Node.js compatibility
  let in_npm_pkg_checker = ByonmInNpmPackageChecker;

  // Create package json resolver first
  let pkg_json_resolver = Arc::new(PackageJsonResolver::<sys_traits::impls::RealSys>::new(
    sys_traits::impls::RealSys,
    None,
  ));

  // Create npm resolver
  let npm_resolver_options = ByonmNpmResolverCreateOptions {
    sys: node_resolver::cache::NodeResolutionSys::<sys_traits::impls::RealSys>::new(
      sys_traits::impls::RealSys,
      None,
    ),
    pkg_json_resolver: pkg_json_resolver.clone(),
    root_node_modules_dir: None,
  };
  let npm_resolver = ByonmNpmResolver::new(npm_resolver_options);

  let node_resolver = Arc::new(deno_node::NodeResolver::new(
    in_npm_pkg_checker,
    DenoIsBuiltInNodeModuleChecker,
    npm_resolver,
    pkg_json_resolver.clone(),
    node_resolver::cache::NodeResolutionSys::<sys_traits::impls::RealSys>::new(
      sys_traits::impls::RealSys,
      None,
    ),
    NodeResolverOptions::default(),
  ));

  // Create a simple node require loader
  struct SimpleNodeRequireLoader;
  impl deno_node::NodeRequireLoader for SimpleNodeRequireLoader {
    fn ensure_read_permission<'a>(
      &self,
      _permissions: &mut dyn deno_node::NodePermissions,
      path: std::borrow::Cow<'a, std::path::Path>,
    ) -> Result<std::borrow::Cow<'a, std::path::Path>, JsErrorBox> {
      Ok(path)
    }

    fn load_text_file_lossy(
      &self,
      path: &std::path::Path,
    ) -> Result<deno_core::FastString, JsErrorBox> {
      let content =
        std::fs::read_to_string(path).map_err(|e| JsErrorBox::new("Error", e.to_string()))?;
      Ok(deno_core::FastString::from(content))
    }

    fn is_maybe_cjs(&self, _url: &deno_core::url::Url) -> Result<bool, ClosestPkgJsonError> {
      Ok(false)
    }
  }

  let node_require_loader = Rc::new(SimpleNodeRequireLoader);
  
  let node_services = deno_node::NodeExtInitServices {
    node_resolver: node_resolver.clone(),
    node_require_loader: node_require_loader.clone(),
    pkg_json_resolver: pkg_json_resolver.clone(),
    sys: sys_traits::impls::RealSys,
  };

  let services = WorkerServiceOptions::<
    ByonmInNpmPackageChecker,
    ByonmNpmResolver<sys_traits::impls::RealSys>,
    sys_traits::impls::RealSys,
  > {
    module_loader,
    permissions: PermissionsContainer::new(permission_desc_parser, permissions),
    blob_store: Default::default(),
    broadcast_channel: Default::default(),
    feature_checker: Default::default(),
    node_services: Some(deno_node::NodeExtInitServices {
      node_resolver: node_resolver.clone(),
      node_require_loader: node_require_loader.clone(),
      pkg_json_resolver: pkg_json_resolver.clone(),
      sys: sys_traits::impls::RealSys,
    }),
    npm_process_state_provider: Default::default(),
    root_cert_store_provider: Default::default(),
    shared_array_buffer_store: Default::default(),
    compiled_wasm_module_store: Default::default(),
    v8_code_cache: Default::default(),
    fs: fs.clone(),
    deno_rt_native_addon_loader: Default::default(),
    fetch_dns_resolver: Default::default(),
  };
  let main_module = deno_core::resolve_path(file_path, &env::current_dir()?)?;

  // Build the bootstrap options that are expected by the runtime
  let bootstrap_options = BootstrapOptions::default();

  // Create an extension that provides SnapshotOptions
  let snapshot_extension = deno_core::Extension {
    name: "snapshot_provider",
    op_state_fn: Some(Box::new(|state| {
      state.put(SnapshotOptions::default());
    })),
    ..Default::default()
  };

  let options = WorkerOptions {
    bootstrap: bootstrap_options,
    extensions: vec![
      snapshot_extension, 
      my_extension::init(),
    ],
    startup_snapshot: None,
    ..Default::default()
  };

  let mut worker = MainWorker::bootstrap_from_options(&main_module, services, options);

  // In `op_scripting_demo`, we borrow the host state struct from the OpState.
  // For this to work, we must first insert the host state into Deno's OpState.
  // Values inside the OpState are identified by their type signature and must be
  // retrieved with the same.
  worker.js_runtime.op_state().borrow_mut().put(host_state);

  // Load Node.js compatibility layer before executing the main module
  let node_compat_code = include_str!("../builtins/node_compat.js");
  worker.execute_script("node_compat", deno_core::FastString::from(node_compat_code))?;

  // We could call `worker.execute_main_module` here, but then we would not be able to access
  // functions exported by the user script.
  // By manually preloading and evaluating the module, we gain access to the internal module id,
  // from which we can extract functions and variables exported by the script.
  let module_id = worker.preload_main_module(&main_module).await?;
  worker.evaluate_module(module_id).await?;
  worker.run_event_loop(false).await?;

  // After the script has been loaded and evaluated, we can access its exports.
  // We retrieve the exported function under the user-provided function name and directly execute
  // it. Theoretically, we could store the function reference for later use and call it in
  // reaction to certain application events.
  let global = get_export_function_global(&mut worker, module_id, fn_name)?;
  let call = worker.js_runtime.call(&global);
  worker
    .js_runtime
    .with_event_loop_promise(call, PollEventLoopOptions::default())
    .await?;

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
