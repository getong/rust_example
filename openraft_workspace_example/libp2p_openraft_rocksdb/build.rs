fn main() -> Result<(), Box<dyn std::error::Error>> {
  let proto_dir = "proto";
  let proto_file = "proto/raft_kv.proto";

  prost_build::Config::new().compile_protos(&[proto_file], &[proto_dir])?;

  println!("cargo:rerun-if-changed={}", proto_file);
  println!("cargo:rerun-if-changed={}", proto_dir);
  Ok(())
}
