use std::{env, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
  let build_out_dir = PathBuf::from(env::var("OUT_DIR")?);
  let workspace_dir = manifest_dir
    .parent()
    .ok_or("game_shared should live inside the workspace")?;
  let proto_file = workspace_dir.join("protobuf/game.proto");
  let proto_dir = workspace_dir.join("protobuf");

  prost_build::Config::new()
    .out_dir(&build_out_dir)
    .compile_protos(&[proto_file], &[proto_dir])?;

  println!(
    "cargo:rerun-if-changed={}",
    workspace_dir.join("protobuf/game.proto").display()
  );
  Ok(())
}
