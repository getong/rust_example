#![allow(clippy::all)]

wasmtime::component::bindgen!({
  path: "wit",
  world: "risk-rule",
  with: {},
  require_store_data_send: true,
});
