fn main() {
  // Platform and V8 initialization
  let platform = v8::Platform::new(0, false).make_shared();
  v8::V8::initialize_platform(platform);
  v8::V8::initialize();
  // `include_str!` is a Rust macro that loads a file and converts it into a Rust string
  let runtime = include_str!("runtime.js");

  let worker_script = r#"
    export function handler(y) {
        return sayHello(y);
    };
    "#;
  // The runtime.js file exposes the `handler` function as a global object
  let script = format!(
    r#"
        {runtime}
        {worker_script}
        "#
  );

  {
    // Create a V8 isolate with default parameters
    let mut isolate = v8::Isolate::new(v8::CreateParams::default());
    let global = setup_runtime(&mut isolate);
    let worker_scope = &mut v8::HandleScope::with_context(isolate.as_mut(), global.clone());
    let handler = build_worker(script.as_str(), worker_scope, &global);
    run_worker(handler, worker_scope, &global);
  }

  unsafe {
    v8::V8::dispose();
  }
  v8::V8::dispose_platform();
}

// Set up the global runtime context
fn setup_runtime(isolate: &mut v8::OwnedIsolate) -> v8::Global<v8::Context> {
  // Create a handle scope for all isolate handles
  let isolate_scope = &mut v8::HandleScope::new(isolate);
  // ObjectTemplate is used to create objects inside the isolate
  let globals = v8::ObjectTemplate::new(isolate_scope);
  // The function name to bind to the Rust implementation
  let resource_name = v8::String::new(isolate_scope, "sayHello").unwrap().into();
  // Expose the function to the global object
  globals.set(
    resource_name,
    v8::FunctionTemplate::new(isolate_scope, say_hello_binding).into(),
  );
  // Create a context for isolate execution
  let context_options = v8::ContextOptions {
    global_template: Some(globals),
    ..Default::default()
  };
  let global_context = v8::Context::new(isolate_scope, context_options);
  // Create and return the global context
  v8::Global::new(isolate_scope, global_context)
}

// Define the Rust binding for the sayHello function
pub fn say_hello_binding(
  scope: &mut v8::HandleScope,
  args: v8::FunctionCallbackArguments,
  mut retval: v8::ReturnValue,
) {
  let to = args.get(0).to_rust_string_lossy(scope);
  let hello = v8::String::new(scope, format!("Hello {}", to).as_str())
    .unwrap()
    .into();
  retval.set(hello);
}

// Build the worker by compiling and instantiating the script
fn build_worker(
  script: &str,
  worker_scope: &mut v8::HandleScope,
  global: &v8::Global<v8::Context>,
) -> v8::Global<v8::Function> {
  let code = v8::String::new(worker_scope, script).unwrap();
  let resource_name = v8::String::new(worker_scope, "script.js").unwrap().into();
  // The source map is optional and used for debugging purposes
  let source_map_url: Option<v8::Local<'_, v8::Value>> =
    Some(v8::String::new(worker_scope, "placeholder").unwrap().into());
  let mut source = v8::script_compiler::Source::new(
    code,
    Some(&v8::ScriptOrigin::new(
      worker_scope,
      resource_name,
      0,
      0,
      false,
      i32::from(0),
      source_map_url,
      false,
      false,
      true,
      None,
    )),
  );
  // Compile and evaluate the module
  let module = v8::script_compiler::compile_module(worker_scope, &mut source).unwrap();
  let _ = module.instantiate_module(worker_scope, |_, _, _, _| None);
  let _ = module.evaluate(worker_scope);
  // open a global scope associated to the worker_scope
  let global = global.open(worker_scope);
  // create and assign the handler to the global context
  let global = global.global(worker_scope);
  let handler_key = v8::String::new(worker_scope, "workerHandler").unwrap();
  let js_handler = global.get(worker_scope, handler_key.into()).unwrap();
  let local_handler = v8::Local::<v8::Function>::try_from(js_handler).unwrap();
  v8::Global::new(worker_scope, local_handler)
}

// Run the worker and execute the `handler` function
pub fn run_worker(
  worker: v8::Global<v8::Function>,
  scope: &mut v8::HandleScope,
  global: &v8::Global<v8::Context>,
) {
  let handler = worker.open(scope);
  let global = global.open(scope);
  let global = global.global(scope);

  let param = v8::String::new(scope, "World").unwrap().into();
  // call the handler and get the result
  match handler.call(scope, global.into(), &[param]) {
    Some(response) => {
      let result =
        v8::Local::<v8::String>::try_from(response).expect("Handler did not return a string");
      let result = result.to_string(scope).unwrap();
      println!("{}", result.to_rust_string_lossy(scope));
    }
    None => todo!(),
  };
}

// copy from https://dev.to/pul/2-daily-rabbit-holes-diving-deeper-into-rust-v8-and-the-javascript-saga-5ajc
// also see https://dzx.cz/2023-03-08/how_do_cloudflare_workers_work/