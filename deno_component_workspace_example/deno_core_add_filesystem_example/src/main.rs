// main.rs
use std::rc::Rc;

use deno_core::{error::AnyError, extension, op2};

#[op2]
#[string]
async fn op_read_file(#[string] path: String) -> Result<String, std::io::Error> {
  tokio::fs::read_to_string(path).await
}

#[op2]
async fn op_write_file(
  #[string] path: String,
  #[string] contents: String,
) -> Result<(), std::io::Error> {
  tokio::fs::write(path, contents).await
}

#[op2(fast)]
fn op_remove_file(#[string] path: String) -> Result<(), std::io::Error> {
  std::fs::remove_file(path)
}

extension!(runjs,
           ops = [
               op_read_file,
               op_write_file,
               op_remove_file,],
             esm_entry_point = "ext:runjs/runtime.js",
             esm = [dir "src", "runtime.js"],
);

async fn run_js(file_path: &str) -> Result<(), AnyError> {
  let main_module = deno_core::resolve_path(file_path, &std::env::current_dir()?)?;
  let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
    module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
    extensions: vec![runjs::init()],
    ..Default::default()
  });

  // let internal_mod_id = js_runtime
  //   .load_side_es_module_from_code(
  //     &deno_core::ModuleSpecifier::parse("runjs:runtime.js")?,
  //     include_str!("runtime.js"),
  //   )
  //   .await?;

  // let internal_mod_result = js_runtime.mod_evaluate(internal_mod_id);

  let mod_id = js_runtime.load_main_es_module(&main_module).await?;
  let result = js_runtime.mod_evaluate(mod_id);
  js_runtime.run_event_loop(Default::default()).await?;
  // internal_mod_result.await?;
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
// also see the source code at https://github.com/denoland/roll-your-own-javascript-runtime
