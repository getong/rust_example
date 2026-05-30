use std::{
  env, fs, io,
  path::{Path, PathBuf},
  process::Command,
};

const TARGET: &str = "wasm32-wasip2";

fn main() {
  println!("cargo:rerun-if-env-changed=KAMEO_SKIP_RULE_BUILD");
  println!("cargo:rerun-if-changed=risk_rule_v1/Cargo.toml");
  println!("cargo:rerun-if-changed=risk_rule_v1/src/lib.rs");
  println!("cargo:rerun-if-changed=risk_rule_v2/Cargo.toml");
  println!("cargo:rerun-if-changed=risk_rule_v2/src/lib.rs");
  println!("cargo:rerun-if-changed=rules/current/risk_rule.wasm");
  println!("cargo:rerun-if-changed=rules/releases/risk_rule_v2.wasm");

  if env::var_os("KAMEO_SKIP_RULE_BUILD").is_some() {
    println!("cargo:warning=skipping wasm rule build because KAMEO_SKIP_RULE_BUILD is set");
    return;
  }

  let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
  let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
  let target_dir = manifest_dir.join("target/wasm-rules");

  build_rule(
    &cargo,
    &manifest_dir,
    &target_dir,
    "risk_rule_v1/Cargo.toml",
  );
  build_rule(
    &cargo,
    &manifest_dir,
    &target_dir,
    "risk_rule_v2/Cargo.toml",
  );

  copy_rule(
    &target_dir
      .join(TARGET)
      .join("release/kameo_risk_rule_v1.wasm"),
    &manifest_dir.join("rules/current/risk_rule.wasm"),
  );
  copy_rule(
    &target_dir
      .join(TARGET)
      .join("release/kameo_risk_rule_v2.wasm"),
    &manifest_dir.join("rules/releases/risk_rule_v2.wasm"),
  );
}

fn build_rule(cargo: &std::ffi::OsStr, manifest_dir: &Path, target_dir: &Path, manifest: &str) {
  let status = Command::new(cargo)
    .arg("build")
    .arg("--manifest-path")
    .arg(manifest_dir.join(manifest))
    .arg("--release")
    .arg("--target")
    .arg(TARGET)
    .arg("--target-dir")
    .arg(target_dir)
    .env("CARGO_ENCODED_RUSTFLAGS", "")
    .env("CARGO_TARGET_WASM32_WASIP2_RUSTFLAGS", "")
    .env("KAMEO_SKIP_RULE_BUILD", "1")
    .env_remove("RUSTFLAGS")
    .status()
    .unwrap_or_else(|err| panic!("failed to start cargo for {manifest}: {err}"));

  if !status.success() {
    panic!(
      "failed to build {manifest} for {TARGET}; install the target with `rustup target add \
       {TARGET}`"
    );
  }
}

fn copy_rule(source: &Path, destination: &Path) {
  if let Some(parent) = destination.parent() {
    fs::create_dir_all(parent).unwrap_or_else(|err| {
      panic!(
        "failed to create wasm rule directory {}: {err}",
        parent.display()
      )
    });
  }

  copy_file(source, destination).unwrap_or_else(|err| {
    panic!(
      "failed to copy wasm rule {} -> {}: {err}",
      source.display(),
      destination.display()
    )
  });
}

fn copy_file(source: &Path, destination: &Path) -> io::Result<()> {
  fs::copy(source, destination).map(|_| ())
}
