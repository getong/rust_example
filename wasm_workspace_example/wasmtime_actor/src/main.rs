#[cfg(target_arch = "wasm32")]
compile_error!("`wasmtime_actor` is the native host binary; the crate lib is the wasm guest.");

#[cfg(not(target_arch = "wasm32"))]
mod bindings;

#[cfg(not(target_arch = "wasm32"))]
mod host;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
  host::run()
}
