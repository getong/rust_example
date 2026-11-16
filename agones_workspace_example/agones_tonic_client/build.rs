fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("cargo:rerun-if-changed=proto");

  let mut config = tonic_prost_build::Config::new();
  config.protoc_executable(protoc_bin_vendored::protoc_bin_path()?);
  let includes = [
    std::path::PathBuf::from("proto"),
    protoc_bin_vendored::include_path()?,
  ];

  tonic_prost_build::configure()
    .build_server(false)
    .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
    .compile_with_config(
      config,
      &[std::path::PathBuf::from("proto/allocation.proto")],
      &includes,
    )?;

  Ok(())
}
