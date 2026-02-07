use std::rc::Rc;

use anyhow::{Context, Result};
use deno_core::{FsModuleLoader, JsRuntime, PollEventLoopOptions, RuntimeOptions, v8};

fn main() -> Result<()> {
  let mut js_runtime = JsRuntime::new(RuntimeOptions {
    module_loader: Some(Rc::new(FsModuleLoader)),
    ..Default::default()
  });

  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()?;

  let main_module = deno_core::resolve_path(
    "./src/module.js",
    &std::env::current_dir().context("Unable to get CWD")?,
  )?;

  let future = async move {
    let mod_id = js_runtime.load_main_es_module(&main_module).await?;
    // let result = js_runtime.mod_evaluate(mod_id);
    js_runtime
      .run_event_loop(PollEventLoopOptions::default())
      .await?;

    let global = js_runtime.get_module_namespace(mod_id).unwrap();
    deno_core::scope!(scope, &mut js_runtime);

    let func_key = v8::String::new(scope, "sum").unwrap();
    let global_obj = v8::Local::new(scope, global);
    let func = global_obj.get(scope, func_key.into()).unwrap();
    let func = v8::Local::<v8::Function>::try_from(func).unwrap();

    let a = v8::Integer::new(scope, 5).into();
    let b = v8::Integer::new(scope, 2).into();
    let func_res = func.call(scope, global_obj.into(), &[a, b]).unwrap();
    let func_res = func_res
      .to_string(scope)
      .unwrap()
      .to_rust_string_lossy(scope);
    println!("Function returned: {}", func_res);

    // result.await?;
    Ok::<(), anyhow::Error>(())
  };
  runtime.block_on(future)?;
  Ok(())
}

// copy from https://stackoverflow.com/questions/76367009/how-to-export-javascript-module-members-to-rust-and-call-them-using-v8-or-deno-c
