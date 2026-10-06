use std::{env, path::Path, process::Command};

fn run_bun(root: &Path, args: &[&str]) {
  let status = Command::new("bun")
    .args(args)
    .current_dir(root)
    // Frontend build tools are devDependencies, including in release builds.
    .env("NODE_ENV", "development")
    .status()
    .unwrap_or_else(|error| {
      panic!("Could not run bun: {error}. Install Bun and make sure it is on PATH.")
    });
  assert!(status.success(), "bun {} failed ({status})", args.join(" "));
}

fn main() {
  for input in [
    "frontend",
    "scripts/build.ts",
    "package.json",
    "bun.lock",
    "tsconfig.json",
    "tests",
    "playwright.config.ts",
  ] {
    println!("cargo:rerun-if-changed={input}");
  }
  println!("cargo:rerun-if-env-changed=PATH");
  let root = env::var("CARGO_MANIFEST_DIR").expect("Cargo CARGO_MANIFEST_DIR");
  let root = Path::new(&root);
  let out = env::var("OUT_DIR").expect("Cargo OUT_DIR");
  run_bun(root, &["install", "--frozen-lockfile"]);
  run_bun(root, &["run", "build", "--outdir", &out]);
}
