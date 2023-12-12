const PROTO_DIR: &str = "protobuf";

fn main() -> std::io::Result<()> {
  let mut protobuf_out = std::path::PathBuf::new();
  // protobuf_out.push(&std::env::var("OUT_DIR").unwrap());
  // proto_path
  protobuf_out.push(&"./src/protobuf");
  std::fs::create_dir(&protobuf_out).ok();

  prost_build::Config::new()
    .out_dir(&protobuf_out)
    .default_package_filename("mod")
    .compile_protos(
      &glob::glob(&(PROTO_DIR.to_string() + "/*.proto"))
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.display().to_string())
        .collect::<Vec<_>>(),
      &[PROTO_DIR],
    )?;

  Ok(())
}
