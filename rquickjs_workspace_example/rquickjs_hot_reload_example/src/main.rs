use notify::{RecursiveMode, Result, Watcher};
use rquickjs::{
  loader::{FileResolver, ScriptLoader},
  Context, Function, Module, Runtime,
};
use std::{
  path::Path,
  sync::{Arc, Mutex},
  thread,
  time::Duration,
};
use tokio::sync::watch;

fn print(msg: String) {
  println!("{}", msg);
}

#[tokio::main]
async fn main() -> Result<()> {
  let (tx, mut rx) = watch::channel(());
  tokio::spawn(async move {
    if let Ok(mut watcher) = notify::recommended_watcher(|res| match res {
      Ok(_event) => {
        _ = tx.send(());
      }
      Err(e) => println!("watch error: {:?}", e),
    }) {
      if let Err(_) = watcher.watch(Path::new("."), RecursiveMode::Recursive) {
        println!("wrong");
      }
    } else {
      println!("wrong");
    }
  });

  let resolver = FileResolver::default().with_path("./");
  let loader = ScriptLoader::default();
  let rt = Runtime::new().unwrap();
  rt.set_loader(resolver, loader);

  let ctx = Context::full(&rt).unwrap();
  ctx.with(|ctx| {
    let global = ctx.globals();
    global
      .set(
        "print",
        Function::new(ctx.clone(), print)
          .unwrap()
          .with_name("print")
          .unwrap(),
      )
      .unwrap();

    println!("Importing script module");
    let mut module_code = load_module_code();
    let module = Module::evaluate(ctx.clone(), "test", module_code.as_bytes()).unwrap();

    let shared_module = Arc::new(Mutex::new(module));
    let shared_module_clone = Arc::clone(&shared_module);

    loop {
      // Access the shared module and execute it
      _ = rx.changed().await;
      let shared_module = shared_module.lock().unwrap();
      shared_module.clone().finish::<()>().unwrap();
      thread::sleep(Duration::from_secs(1)); // Adjust the sleep duration as needed
    }
  });
  Ok(())
}

fn load_module_code() -> String {
  // Load your module code from a file or any other source
  // For simplicity, I'll return a static module code here
  r#"
        import { n, s, f } from "script_module";
        print(`n = ${n}`);
        print(`s = "${s}"`);
        print(`f(2, 4) = ${f(2, 4)}`);
    "#
  .to_string()
}
