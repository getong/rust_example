use std::{path::Path, process::Command};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let proto_dir = "proto";
  let proto_files = ["proto/raft_kv.proto", "proto/chat.proto"];

  prost_build::Config::new().compile_protos(&proto_files, &[proto_dir])?;

  for proto_file in proto_files {
    println!("cargo:rerun-if-changed={}", proto_file);
  }
  println!("cargo:rerun-if-changed={}", proto_dir);

  build_typed_hello_guest();
  Ok(())
}

/// Auto-build the example typed wasm guest (wasm-optimization-v2.md §7,
/// pattern from `kameo_wasmtime_hot_upgrade/build.rs`) so the typed-path
/// e2e tests always exercise a guest built from current sources. BEST
/// EFFORT: a missing wasm32-wasip2 target or an offline registry only
/// prints a warning — the cluster build itself must never fail over the
/// example guest (its e2e test skips when the artifact is absent).
fn build_typed_hello_guest() {
  println!("cargo:rerun-if-changed=wasm_guests/typed_hello/src/lib.rs");
  println!("cargo:rerun-if-changed=wasm_guests/typed_hello/Cargo.toml");
  println!("cargo:rerun-if-changed=wit/task-executor.wit");

  let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
  let guest_dir = Path::new(&manifest_dir).join("wasm_guests/typed_hello");
  if !guest_dir.join("Cargo.toml").is_file() {
    return;
  }
  let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

  // The guest is its own workspace with its own target dir; scrub the
  // host-build env so host RUSTFLAGS / target-dir redirection don't leak
  // into the wasm cross-build (and can't deadlock on our target dir).
  let status = Command::new(cargo)
    .current_dir(&guest_dir)
    .env_remove("CARGO_TARGET_DIR")
    .env_remove("RUSTFLAGS")
    .env_remove("CARGO_ENCODED_RUSTFLAGS")
    .args(["build", "--release", "--target", "wasm32-wasip2"])
    .status();

  match status {
    Ok(status) if status.success() => {}
    Ok(status) => println!(
      "cargo:warning=typed_hello wasm guest build failed ({status}); typed-guest e2e tests will \
       be skipped (rustup target add wasm32-wasip2)"
    ),
    Err(err) => println!(
      "cargo:warning=typed_hello wasm guest build not run ({err}); typed-guest e2e tests will be \
       skipped"
    ),
  }
}
