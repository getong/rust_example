use rquickjs::{
  loader::{FileResolver, ScriptLoader},
  Context, Function, Module, Runtime,
};

fn print(msg: String) {
  println!("{msg}");
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

    println!("import script module");
    Module::evaluate(
      ctx.clone(),
      "test",
      r#"
import { n, s, f } from "script_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
    )
    .unwrap()
    .finish::<()>()
    .unwrap();
  });
}
