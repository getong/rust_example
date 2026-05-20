fn main() -> Result<(), Box<dyn std::error::Error>> {
  println!("cargo:rerun-if-changed=src/message.proto");

  prost_build::Config::new().compile_protos(&["src/message.proto"], &["src"])?;

  Ok(())
}
