// main.rs
use std::rc::Rc;

use deno_core::error::AnyError;

async fn run_js(file_path: &str) -> Result<(), AnyError> {
  let main_module = deno_core::resolve_path(file_path, &std::env::current_dir()?)?;
  let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
    module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
    ..Default::default()
  });

  let mod_id = js_runtime.load_main_es_module(&main_module).await?;
  let result = js_runtime.mod_evaluate(mod_id);
  js_runtime.run_event_loop(Default::default()).await?;
  result.await.map_err(AnyError::from)
}

fn main() {
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();
  if let Err(error) = runtime.block_on(run_js("src/example.js")) {
    eprintln!("error: {}", error);
  }
}

// copy from https://deno.com/blog/roll-your-own-javascript-runtime