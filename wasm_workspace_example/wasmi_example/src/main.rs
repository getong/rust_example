use wasmi::{Engine, Instance, Module, Store, *};

fn main() {
  let wasm = r#"
        (module
            (func (export "test") (result i32)
                i32.const 1337
            )
        )
    "#;

  let engine = Engine::default();
  let module = Module::new(&engine, wasm).expect("failed to load wasm");
  let mut store = Store::new(&engine, ());
  let instance =
    Instance::new(&mut store, &module, &[]).expect("failed to instantiate wasm module");
  let test = instance
    .get_typed_func::<(), i32>(&store, "test")
    .expect("failed to find exported function");

  assert_eq!(
    test.call(&mut store, ()).expect("failed to execute export"),
    1337
  );
  _ = wasmi_function().unwrap();
}

fn wasmi_function() -> Result<(), wasmi::Error> {
  let wasm = r#"
        (module
            (import "host" "hello" (func $host_hello (param i32)))
            (func (export "hello")
                (call $host_hello (i32.const 3))
            )
        )
    "#;
  // First step is to create the Wasm execution engine with some config.
  //
  // In this example we are using the default configuration.
  let engine = Engine::default();
  // Now we can compile the above Wasm module with the given Wasm source.
  let module = Module::new(&engine, wasm)?;

  // Wasm objects operate within the context of a Wasm `Store`.
  //
  // Each `Store` has a type parameter to store host specific data.
  // In this example the host state is a simple `u32` type with value `42`.
  type HostState = u32;
  let mut store = Store::new(&engine, 42);

  // A linker can be used to instantiate Wasm modules.
  // The job of a linker is to satisfy the Wasm module's imports.
  let mut linker = <Linker<HostState>>::new(&engine);
  // We are required to define all imports before instantiating a Wasm module.
  _ = linker.func_wrap(
    "host",
    "hello",
    |caller: Caller<'_, HostState>, param: i32| {
      println!(
        "Got {param} from WebAssembly and my host state is: {}",
        caller.data()
      );
    },
  );
  let instance = linker.instantiate_and_start(&mut store, &module)?;
  // Now we can finally query the exported "hello" function and call it.
  instance
    .get_typed_func::<(), ()>(&store, "hello")?
    .call(&mut store, ())?;
  Ok(())
}
// copy from https://docs.rs/wasmi/1.0.9/wasmi/
