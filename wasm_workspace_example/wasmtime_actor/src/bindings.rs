wasmtime::component::bindgen!({
  path: "src/wit",
  world: "actor-world",
  require_store_data_send: true,
});
