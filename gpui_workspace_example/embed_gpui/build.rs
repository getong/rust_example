use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
  println!("cargo:rerun-if-changed=app");

  let app = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("missing CARGO_MANIFEST_DIR")?)
    .join("app");
  let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("missing OUT_DIR")?).join("app");
  let ui_sets = vec![nsh_ui_mobile::registry()];
  // Use the same bundler and bytecode compiler as `nsh build`, without
  // requiring an installed CLI or invoking Cargo recursively.
  let bundle = nsh_bundler::bundle(&nsh_bundler::BundleOptions {
    entry: app.join("App.tsx"),
    minify: false,
    ui_sets: ui_sets.clone(),
  })?;
  let bytecode = nsh_js::compile_bytecode_with(&bundle.code, false, &ui_sets)?;
  fs::create_dir_all(&out)?;
  fs::write(out.join("bundle.nshc"), bytecode)?;
  fs::write(out.join("bundle.js"), &bundle.code)?;
  fs::write(
    out.join("bundle.js.map"),
    bundle.source_map.as_deref().unwrap_or_default(),
  )?;
  let capabilities = out.join("capabilities.json");
  if let Some(declared) = bundle.capabilities {
    fs::write(capabilities, declared.to_json().to_string())?;
  } else if capabilities.exists() {
    fs::remove_file(capabilities)?;
  }
  for asset in &bundle.assets {
    let source = app.join(asset);
    println!("cargo:rerun-if-changed={}", source.display());
    let dest = out.join("assets").join(asset);
    if let Some(parent) = dest.parent() {
      fs::create_dir_all(parent)?;
    }
    fs::copy(source, dest)?;
  }
  for set in &ui_sets {
    for asset in set.asset_files() {
      println!("cargo:rerun-if-changed={}", asset.source);
      let dest = out.join("assets").join(asset.path());
      if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
      }
      fs::copy(asset.source, dest)?;
    }
  }
  Ok(())
}
