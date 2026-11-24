use std::{env, path::PathBuf};

use rquickjs::{
  CatchResultExt, Context, Ctx, Error, Function, Module, Runtime,
  loader::{
    BuiltinLoader, BuiltinResolver, FileResolver, ImportAttributes, ModuleLoader, NativeLoader,
    Resolver, ScriptLoader,
  },
};

mod bundle;
use bundle::{NativeModule, SCRIPT_MODULE};

fn print(msg: String) {
  println!("{msg}");
}

struct NativeModuleResolver {
  path: PathBuf,
}

impl Resolver for NativeModuleResolver {
  fn resolve<'js>(
    &mut self,
    _ctx: &Ctx<'js>,
    base: &str,
    name: &str,
    _attributes: Option<ImportAttributes<'js>>,
  ) -> rquickjs::Result<String> {
    if name != "native_module_example" {
      return Err(Error::new_resolving(base, name));
    }

    self.path.to_str().map(str::to_owned).ok_or_else(|| {
      Error::new_resolving_message(base, name, "native module path is not valid UTF-8")
    })
  }
}

fn evaluate_module(ctx: &Ctx<'_>, name: &str, source: &str) -> Result<(), String> {
  Module::evaluate(ctx.clone(), name, source)
    .and_then(|promise| promise.finish::<()>())
    .catch(ctx)
    .map_err(|error| format!("{name}: {error}"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let native_module_path = env::current_exe()?.with_file_name(format!(
    "{}native_module_example{}",
    env::consts::DLL_PREFIX,
    env::consts::DLL_SUFFIX,
  ));
  if !native_module_path.is_file() {
    return Err(
      format!(
        "native module not found at {}; run `cargo build -p native_module_example` with the same \
         target directory and profile first",
        native_module_path.display(),
      )
      .into(),
    );
  }

  let resolver = (
    BuiltinResolver::default()
      .with_module("bundle/script_module")
      .with_module("bundle/native_module"),
    NativeModuleResolver {
      path: native_module_path,
    },
    FileResolver::default().with_path("./").with_native(),
  );
  let loader = (
    BuiltinLoader::default().with_module("bundle/script_module", SCRIPT_MODULE),
    ModuleLoader::default().with_module("bundle/native_module", NativeModule),
    ScriptLoader::default(),
    NativeLoader::default(),
  );

  let rt = Runtime::new()?;
  let ctx = Context::full(&rt)?;

  rt.set_loader(resolver, loader);
  ctx.with(|ctx| -> Result<(), Box<dyn std::error::Error>> {
    let global = ctx.globals();
    global.set(
      "print",
      Function::new(ctx.clone(), print)?.with_name("print")?,
    )?;

    println!("import script module");
    evaluate_module(
      &ctx,
      "test_script_module",
      r#"
import { n, s, f } from "script_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
    )?;

    println!("import native module");
    evaluate_module(
      &ctx,
      "test_native_module",
      r#"
import { n, s, f } from "native_module_example";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
                "#,
    )?;

    println!("import bundled script module");
    evaluate_module(
      &ctx,
      "test_bundled_script_module",
      r#"
import { n, s, f } from "bundle/script_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
    )?;

    println!("import bundled native module");
    evaluate_module(
      &ctx,
      "test_bundled_native_module",
      r#"
import { n, s, f } from "bundle/native_module";
print(`n = ${n}`);
print(`s = "${s}"`);
print(`f(2, 4) = ${f(2, 4)}`);
"#,
    )?;
    Ok(())
  })
}
