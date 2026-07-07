use std::{
  env, fs,
  path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
  let build_out_dir = PathBuf::from(env::var("OUT_DIR")?);
  let workspace_dir = manifest_dir
    .parent()
    .ok_or("game_server should live inside the workspace")?;
  let proto_file = workspace_dir.join("protobuf/game.proto");
  let proto_dir = workspace_dir.join("protobuf");
  let source_out_dir = manifest_dir.join("src/protocol/generated");
  let generated_source_file = source_out_dir.join("game.rs");

  fs::create_dir_all(&source_out_dir)?;

  prost_build::Config::new()
    .out_dir(&build_out_dir)
    .compile_protos(&[proto_file], &[proto_dir])?;

  copy_if_changed(&build_out_dir.join("game.rs"), &generated_source_file)?;

  println!(
    "cargo:rerun-if-changed={}",
    workspace_dir.join("protobuf/game.proto").display()
  );
  println!("cargo:rerun-if-changed={}", generated_source_file.display());
  Ok(())
}

fn copy_if_changed(source: &Path, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
  let source_contents = fs::read(source)?;
  if fs::read(destination).ok().as_deref() == Some(source_contents.as_slice()) {
    return Ok(());
  }

  fs::write(destination, source_contents)?;
  Ok(())
}
