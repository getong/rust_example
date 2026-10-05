use std::{env, process::Command};

fn main() {
  for input in [
    "frontend",
    "scripts/build.ts",
    "package.json",
    "package-lock.json",
    "tsconfig.json",
    "tests",
    "playwright.config.ts",
  ] {
    println!("cargo:rerun-if-changed={input}");
  }
  let out = env::var("OUT_DIR").expect("Cargo OUT_DIR");
  let status = Command::new("npm")
    .args(["run", "build", "--", "--outdir", &out])
    .status()
    .expect("TypeScript build requires Node.js and npm. Run npm ci in topcoat_gpui_example first.");
  assert!(
    status.success(),
    "TypeScript build failed. Run npm ci and npm run typecheck in topcoat_gpui_example."
  );
}
