#![allow(clippy::all)]

wasmtime::component::bindgen!({
  path: "wit/risk-rule.wit",
  world: "risk-rule",
  with: {},
});
