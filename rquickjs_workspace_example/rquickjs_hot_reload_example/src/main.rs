use notify::{RecursiveMode, Result, Watcher};
use rquickjs::{
  async_with,
  loader::{FileResolver, ScriptLoader},
  AsyncContext, AsyncRuntime, Function,
};
use std::{path::Path, time::Duration};
use tokio::{fs, sync::watch, time::sleep};

const FILE_NAME: &str = "script_module.js";

fn print(msg: String) {
  println!("{}", msg);
}

#[tokio::main]
async fn main() -> Result<()> {
  let (tx, mut rx) = watch::channel(());
  tokio::spawn(async move {
    if let Ok(mut watcher) = notify::recommended_watcher(move |res| match res {
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
  let rt = AsyncRuntime::new().unwrap();
  rt.set_loader(resolver, loader).await;

  let ctx = AsyncContext::full(&rt).await.unwrap();
  async_with!(&ctx => |ctx| {
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
    let mut module_code =
      if let Ok(file_content) = fs::read_to_string(FILE_NAME).await {
        file_content
      } else {
        String::new()
      };

    loop {
      tokio::select!{
        Ok(_) = rx.changed() => {
          println!("file changed");
          if let Ok(file_content) = fs::read_to_string(FILE_NAME).await{
            module_code = file_content;
          }
        },

        _ = sleep(Duration::from_secs(0)) => {
          if let Ok(res) = ctx.eval::<(), &str>("console.log('hello world');") {
            println!("helloworld_print Result: {:?}", res);
          } else {
            println!("hellowrold_print Failed to evaluate JavaScript code");
          }
          if !module_code.is_empty() {
            println!("&*code_str is {}", &*module_code);
            if let Ok(res) = ctx.eval::<(), &str>(&*module_code) {
              println!("Result: {:?}", res);
            } else {
              println!("Failed to evaluate JavaScript code");
            }
          }
        },
      }
    }
  })
  .await;
  Ok(())
}
