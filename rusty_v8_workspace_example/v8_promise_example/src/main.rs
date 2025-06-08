use std::{fs, path::Path};

use futures::executor::block_on;
use v8::V8;

fn main() {
  let platform = v8::new_default_platform(0, false).make_shared();
  V8::initialize_platform(platform);
  V8::initialize();

  {
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());

    let handle_scope = &mut v8::HandleScope::new(isolate);

    let context = v8::Context::new(handle_scope, v8::ContextOptions::default());
    let scope = &mut v8::ContextScope::new(handle_scope, context);

    let file_path = Path::new("src/index.js");
    let code = fs::read_to_string(file_path).expect("Unable to read JavaScript file.");

    let source = v8::String::new(scope, &code).unwrap();
    let script = v8::Script::compile(scope, source, None).unwrap();
    let _ = script.run(scope).unwrap();

    let global = context.global(scope);
    let add_key = v8::String::new(scope, "add").unwrap();
    let add_val = global.get(scope, add_key.into()).unwrap();
    let add_func = v8::Local::<v8::Function>::try_from(add_val).expect("Function not found.");

    let arg_a = v8::Number::new(scope, 20.0);
    let arg_b = v8::Number::new(scope, 12.0);
    let args = &[arg_a.into(), arg_b.into()];

    let call_func = add_func.call(scope, global.into(), args).unwrap();

    let result = block_on(async {
      let promise = v8::Local::<v8::Promise>::try_from(call_func).unwrap();
      while promise.state() == v8::PromiseState::Pending {
        scope.perform_microtask_checkpoint();
      }
      promise
    });

    let result_str = result.result(scope).to_string(scope).unwrap();
    println!("Result: {}", result_str.to_rust_string_lossy(scope));
    // => Result: 32
  }

  unsafe {
    V8::dispose();
  }
  V8::dispose_platform();
}
