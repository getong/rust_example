use anyhow::Result;
use wasmtime::*;

fn main() -> Result<()> {
  // Modules can be compiled through either the text or binary format
  let engine = Engine::default();
  let wat = r#"
        (module
            (import "host" "host_func" (func $host_hello (param i32)))

            (func (export "hello")
                i32.const 3
                call $host_hello)
        )
    "#;
  let module = Module::new(&engine, wat)?;

  // Create a `Linker` which will be later used to instantiate this module.
  // Host functionality is defined by name within the `Linker`.
  let mut linker = Linker::new(&engine);
  linker.func_wrap(
    "host",
    "host_func",
    |caller: Caller<'_, u32>, param: i32| {
      println!("Got {} from WebAssembly", param);
      println!("my host state is: {}", caller.data());
    },
  )?;

  // All wasm objects operate within the context of a "store". Each
  // `Store` has a type parameter to store host-specific data, which in
  // this case we're using `4` for.
  let mut store = Store::new(&engine, 4);
  let instance = linker.instantiate(&mut store, &module)?;
  let hello = instance.get_typed_func::<(), ()>(&mut store, "hello")?;

  // And finally we can call the wasm!
  hello.call(&mut store, ())?;

  Ok(())
}
