use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
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

fn print(msg: String) {
  println!("{}", msg);
}

fn main() {
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

    // Spawn a thread to watch for file changes
    let shared_module_clone = Arc::clone(&shared_module);
    thread::spawn(move || {
      let (tx, rx) = std::sync::mpsc::channel();
      let config = Config::default()
        .with_poll_interval(Duration::from_secs(2))
        .with_compare_contents(true);
      let mut watcher: RecommendedWatcher = Watcher::new(tx, config).unwrap(); // Configure with zero delay for notifications
      watcher
        .watch(Path::new("./"), RecursiveMode::Recursive)
        .expect("Failed to watch directory");

      loop {
        if let Ok(event) = rx.recv() {
          println!("File change detected: {:?}", event);
          // Reload the module
          let mut module_code = load_module_code();
          let new_module = Module::evaluate(ctx.clone(), "test", module_code.as_bytes()).unwrap();
          let mut shared_module = shared_module_clone.lock().unwrap();
          *shared_module = new_module;
        }
      }
    });

    loop {
      // Access the shared module and execute it
      let shared_module = shared_module.lock().unwrap();
      shared_module.clone().finish::<()>().unwrap();
      thread::sleep(Duration::from_secs(1)); // Adjust the sleep duration as needed
    }
  });
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
