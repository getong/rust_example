// main.rs

static RUNTIME_SNAPSHOT: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/RUNJS_SNAPSHOT.bin"));

use std::rc::Rc;

use deno_ast::{MediaType, ParseParams};
use deno_core::{
  ModuleLoadResponse, ModuleSourceCode,
  error::{AnyError, ModuleLoaderError},
  extension, op2,
};
use deno_error::JsErrorBox;

struct TsModuleLoader;

impl deno_core::ModuleLoader for TsModuleLoader {
  fn resolve(
    &self,
    specifier: &str,
    referrer: &str,
    _kind: deno_core::ResolutionKind,
  ) -> Result<deno_core::ModuleSpecifier, ModuleLoaderError> {
    deno_core::resolve_import(specifier, referrer).map_err(JsErrorBox::from_err)
  }

  fn load(
    &self,
    module_specifier: &deno_core::ModuleSpecifier,
    _maybe_referrer: Option<&deno_core::ModuleLoadReferrer>,
    _options: deno_core::ModuleLoadOptions,
  ) -> ModuleLoadResponse {
    let module_specifier = module_specifier.clone();

    let module_load = move || {
      let path = module_specifier.to_file_path().unwrap();

      let media_type = MediaType::from_path(&path);
      let (module_type, should_transpile) = match MediaType::from_path(&path) {
        MediaType::JavaScript | MediaType::Mjs | MediaType::Cjs => {
          (deno_core::ModuleType::JavaScript, false)
        }
        MediaType::Jsx
        | MediaType::TypeScript
        | MediaType::Mts
        | MediaType::Cts
        | MediaType::Dts
        | MediaType::Dmts
        | MediaType::Dcts
        | MediaType::Tsx => (deno_core::ModuleType::JavaScript, true),
        MediaType::Json => (deno_core::ModuleType::Json, false),
        _ => panic!("Unknown extension {:?}", path.extension()),
      };

      let code = std::fs::read_to_string(&path).map_err(JsErrorBox::from_err)?;
      let code = if should_transpile {
        let parsed = deno_ast::parse_module(ParseParams {
          specifier: module_specifier.clone(),
          text: code.into(),
          media_type,
          capture_tokens: false,
          scope_analysis: false,
          maybe_syntax: None,
        })
        .map_err(JsErrorBox::from_err)?;
        parsed
          .transpile(
            &Default::default(),
            &Default::default(),
            &Default::default(),
          )
          .map_err(JsErrorBox::from_err)?
          .into_source()
          .text
      } else {
        code
      };
      let module = deno_core::ModuleSource::new(
        module_type,
        ModuleSourceCode::String(code.into()),
        &module_specifier,
        None,
      );
      Ok(module)
    };

    ModuleLoadResponse::Sync(module_load())
  }
}

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

#[op2]
#[string]
async fn op_fetch(#[string] url: String) -> Result<String, JsErrorBox> {
  reqwest::get(url)
    .await
    .map_err(|e| JsErrorBox::type_error(e.to_string()))?
    .text()
    .await
    .map_err(|e| JsErrorBox::type_error(e.to_string()))
}

extension!(
  runjs,
  ops = [op_read_file, op_write_file, op_remove_file, op_fetch,],
);

async fn run_js(file_path: &str) -> Result<(), AnyError> {
  let main_module = deno_core::resolve_path(file_path, &std::env::current_dir()?)?;
  let mut js_runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
    module_loader: Some(Rc::new(TsModuleLoader)),
    startup_snapshot: Some(RUNTIME_SNAPSHOT),
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
  let args: Vec<String> = std::env::args().collect();

  if args.is_empty() {
    eprintln!("Usage: cargo run <file>");
    std::process::exit(1);
  }
  let file_path = &args[1];

  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .unwrap();

  if let Err(error) = runtime.block_on(run_js(file_path)) {
    eprintln!("error: {error}");
  }
}

// cargo run src/example.ts

// copy from https://deno.com/blog/roll-your-own-javascript-runtime-pt3
// also see the source code at https://github.com/denoland/roll-your-own-javascript-runtime
