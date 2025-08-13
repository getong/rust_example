#![allow(clippy::print_stdout)]
#![allow(clippy::print_stderr)]

mod module_loader;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use deno_runtime::deno_core::error::AnyError;
use deno_runtime::deno_core::op2;
use deno_runtime::deno_core::JsRuntime;
use deno_runtime::deno_core::ModuleSpecifier;
use deno_runtime::deno_core::RuntimeOptions;

use module_loader::TypescriptModuleLoader;

#[op2]
#[string]
fn example_custom_op(#[string] text: &str) -> String {
    println!("Hello {} from an op!", text);
    text.to_string() + " from Rust!"
}

deno_runtime::deno_core::extension!(
    example_extension,
    ops = [example_custom_op],
);

#[tokio::main]
async fn main() -> Result<(), AnyError> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: deno-runtime-test <js-file>");
        std::process::exit(1);
    }

    let js_path = &args[1];
    let main_module = ModuleSpecifier::from_file_path(Path::new(js_path).canonicalize()?)
        .map_err(|_| anyhow::anyhow!("Failed to create module specifier"))?;
    let source_map_store = Rc::new(RefCell::new(HashMap::new()));

    let mut runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(TypescriptModuleLoader {
            source_maps: source_map_store,
        })),
        extensions: vec![example_extension::init()],
        ..Default::default()
    });

    // Execute the main module
    let mod_id = runtime.load_main_es_module(&main_module).await?;
    let result = runtime.mod_evaluate(mod_id);
    runtime.run_event_loop(Default::default()).await?;
    result.await?;

    Ok(())
}