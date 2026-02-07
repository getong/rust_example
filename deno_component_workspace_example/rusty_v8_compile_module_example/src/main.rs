// Minimal module compilation example updated for v8 145.

fn main() {
  let platform = v8::new_default_platform(0, false).make_shared();
  v8::V8::initialize_platform(platform);
  v8::V8::initialize();

  let mut isolate = v8::Isolate::new(v8::CreateParams::default());
  v8::scope!(let scope, &mut isolate);
  let context = v8::Context::new(&scope, Default::default());
  let scope = &mut v8::ContextScope::new(scope, context);

  let source_code = r#"
    export function add(a, b) { return a + b; }
    globalThis.result = add(20, 12);
  "#;

  let code = v8::String::new(scope, source_code).unwrap();
  let resource_name = v8::String::new(scope, "mod.js").unwrap().into();
  let origin = v8::ScriptOrigin::new(
    scope,
    resource_name,
    0,
    0,
    false,
    0,
    None,
    false,
    false,
    true,
    None,
  );
  let mut source = v8::script_compiler::Source::new(code, Some(&origin));
  let module = v8::script_compiler::compile_module(scope, &mut source).unwrap();
  module.instantiate_module(scope, |_, _, _, _| None).unwrap();
  module.evaluate(scope).unwrap();

  let global = context.global(scope);
  let key = v8::String::new(scope, "result").unwrap();
  let val = global.get(scope, key.into()).unwrap();
  let num = v8::Local::<v8::Number>::try_from(val).unwrap();
  println!("result: {}", num.value());

  unsafe {
    v8::V8::dispose();
  }
  v8::V8::dispose_platform();
}
