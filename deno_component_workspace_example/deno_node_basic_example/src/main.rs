use std::path::PathBuf;

use deno_core::{Extension, JsRuntime, RuntimeOptions};
use deno_web::InMemoryBroadcastChannel;
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

  fn resolve_types_package_folder(
    &self,
    _types_package_name: &str,
    _maybe_package_version: Option<&deno_semver::Version>,
    _maybe_referrer: Option<&node_resolver::UrlOrPathRef<'_>>,
  ) -> Option<PathBuf> {
    None
  }
}

fn main() {
  let rt = tokio::runtime::Runtime::new().unwrap();
  rt.block_on(async_main());
}

async fn async_main() {
  // Create the filesystem implementation
  let fs = deno_fs::sync::new_rc(deno_fs::RealFs) as deno_fs::FileSystemRc;

  // Create comprehensive extensions vector similar to Deno's snapshot
  let extensions: Vec<Extension> = vec![
    deno_telemetry::deno_telemetry::init(),
    deno_webidl::deno_webidl::init(),
    // deno_console/deno_url are deprecated stubs in current versions
    deno_web::deno_web::init(
      Default::default(),
      Default::default(),
      InMemoryBroadcastChannel::default(),
    ),
    deno_webgpu::deno_webgpu::init(),
    deno_fetch::deno_fetch::init(deno_fetch::Options::default()),
    deno_cache::deno_cache::init(None),
    deno_websocket::deno_websocket::init(),
    deno_webstorage::deno_webstorage::init(None),
    deno_crypto::deno_crypto::init(None),
    deno_ffi::deno_ffi::init(None),
    deno_net::deno_net::init(None, None),
    deno_tls::deno_tls::init(),
    deno_kv::deno_kv::init(
      deno_kv::sqlite::SqliteDbHandler::new(None, None),
      deno_kv::KvConfig::builder().build(),
    ),
    deno_cron::deno_cron::init(deno_cron::local::LocalCronHandler::new()),
    deno_napi::deno_napi::init(None),
    deno_http::deno_http::init(deno_http::Options::default()),
    deno_io::deno_io::init(Default::default()),
    deno_fs::deno_fs::init(fs.clone()),
    deno_os::deno_os::init(Default::default()),
    deno_process::deno_process::init(Default::default()),
    deno_node::deno_node::init::<
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

  // Read and execute the main.ts file
  println!("Reading main.ts file...");
  match std::fs::read_to_string("main.ts") {
    Ok(typescript_code) => {
      println!("Executing main.ts...");

      // Since we don't have TypeScript compilation set up, we'll treat it as JavaScript
      // For a full TypeScript setup, you'd need to add deno_ast or swc for compilation
      let result = js_runtime.execute_script("main.ts", typescript_code);

      match result {
        Ok(_) => {
          println!("main.ts executed successfully!");

          // Run the event loop to handle async operations like the HTTP server
          println!("Starting event loop to handle async operations...");

          // Create a waker and context for polling
          use std::task::{Context, Poll};
          // use std::future::Future;
          // use std::pin::Pin;

          let waker = futures::task::noop_waker();
          let mut cx = Context::from_waker(&waker);

          loop {
            match js_runtime.poll_event_loop(&mut cx, Default::default()) {
              Poll::Ready(Ok(())) => {
                println!("Event loop completed successfully");
                break;
              }
              Poll::Ready(Err(e)) => {
                eprintln!("Event loop error: {}", e);
                break;
              }
              Poll::Pending => {
                // Would normally yield here in an async context
                std::thread::sleep(std::time::Duration::from_millis(10));
                continue;
              }
            }
          }
        }
        Err(e) => eprintln!("Failed to execute main.ts: {}", e),
      }
    }
    Err(e) => eprintln!("Failed to read main.ts: {}", e),
  }
}
