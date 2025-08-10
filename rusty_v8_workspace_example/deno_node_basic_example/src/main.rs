use std::{path::PathBuf, rc::Rc};

use deno_core::{Extension, JsRuntime, RuntimeOptions};
use deno_permissions::PermissionsContainer;
use node_resolver::{
  InNpmPackageChecker, NpmPackageFolderResolver, errors::PackageFolderResolveError,
};

// Simple implementations for the required traits
#[derive(Clone)]
pub struct NoopInNpmPackageChecker;

impl InNpmPackageChecker for NoopInNpmPackageChecker {
  fn in_npm_package(&self, _url: &deno_core::url::Url) -> bool {
    false
  }
}

#[derive(Clone)]
pub struct NoopNpmPackageFolderResolver;

impl NpmPackageFolderResolver for NoopNpmPackageFolderResolver {
  fn resolve_package_folder_from_package(
    &self,
    name: &str,
    _referrer: &node_resolver::UrlOrPathRef,
  ) -> Result<PathBuf, PackageFolderResolveError> {
    // Create a simple UrlOrPath since we can't easily convert UrlOrPathRef
    let simple_referrer = node_resolver::UrlOrPath::Path(std::path::PathBuf::from("unknown"));

    let package_io_error = node_resolver::errors::PackageFolderResolveIoError {
      package_name: name.to_string(),
      referrer: simple_referrer,
      source: std::io::Error::new(std::io::ErrorKind::NotFound, "npm packages not supported"),
    };

    Err(PackageFolderResolveError::from(package_io_error))
  }
}

fn main() {
  // Create the filesystem implementation
  let fs = Rc::new(deno_fs::RealFs);

  // Create comprehensive extensions vector similar to Deno's snapshot
  let extensions: Vec<Extension> = vec![
    deno_telemetry::deno_telemetry::init(),
    deno_webidl::deno_webidl::init(),
    deno_console::deno_console::init(),
    deno_url::deno_url::init(),
    deno_web::deno_web::init::<PermissionsContainer>(Default::default(), Default::default()),
    deno_webgpu::deno_webgpu::init(),
    deno_canvas::deno_canvas::init(),
    deno_fetch::deno_fetch::init::<PermissionsContainer>(Default::default()),
    deno_cache::deno_cache::init(None),
    deno_websocket::deno_websocket::init::<PermissionsContainer>("".to_owned(), None, None),
    deno_webstorage::deno_webstorage::init(None),
    deno_crypto::deno_crypto::init(None),
    deno_broadcast_channel::deno_broadcast_channel::init(
      deno_broadcast_channel::InMemoryBroadcastChannel::default(),
    ),
    deno_ffi::deno_ffi::init::<PermissionsContainer>(None),
    deno_net::deno_net::init::<PermissionsContainer>(None, None),
    deno_tls::deno_tls::init(),
    deno_kv::deno_kv::init(
      deno_kv::sqlite::SqliteDbHandler::<PermissionsContainer>::new(None, None),
      deno_kv::KvConfig::builder().build(),
    ),
    deno_cron::deno_cron::init(deno_cron::local::LocalCronHandler::new()),
    deno_napi::deno_napi::init::<PermissionsContainer>(None),
    deno_http::deno_http::init(deno_http::Options::default()),
    deno_io::deno_io::init(Default::default()),
    deno_fs::deno_fs::init::<PermissionsContainer>(fs.clone()),
    deno_os::deno_os::init(Default::default()),
    deno_process::deno_process::init(Default::default()),
    deno_node::deno_node::init::<
      PermissionsContainer,
      NoopInNpmPackageChecker,
      NoopNpmPackageFolderResolver,
      sys_traits::impls::RealSys,
    >(None, fs.clone()),
  ];

  let mut js_runtime = JsRuntime::new(RuntimeOptions {
    extensions,
    ..Default::default()
  });

  println!("Deno runtime initialized with comprehensive extensions!");

  // Execute a simple test to make sure the runtime is working
  let result = js_runtime.execute_script(
    "test.js",
    "console.log('Hello from Deno runtime with comprehensive extensions!'); 'success'",
  );

  match result {
    Ok(_) => println!("Runtime test successful!"),
    Err(e) => println!("Runtime test failed: {}", e),
  }
}
