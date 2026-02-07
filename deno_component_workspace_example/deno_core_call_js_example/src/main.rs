use std::fs;

use deno_core::{JsRuntime, v8};

fn main() {
  let source_code = fs::read_to_string("./src/index.js").unwrap();

  let mut runtime = JsRuntime::new(Default::default());

  let handle = runtime.execute_script("hello", source_code).unwrap();
  deno_core::scope!(scope, &mut runtime);
  let value = v8::Local::new(scope, handle);
  let value = value.to_rust_string_lossy(scope);
  println!("{}", value)
}
