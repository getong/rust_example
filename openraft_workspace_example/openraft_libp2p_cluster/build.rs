fn main() -> Result<(), Box<dyn std::error::Error>> {
  let proto_dir = "proto";
  let proto_files = ["proto/raft_kv.proto", "proto/chat.proto"];

  prost_build::Config::new().compile_protos(&proto_files, &[proto_dir])?;

  for proto_file in proto_files {
    println!("cargo:rerun-if-changed={}", proto_file);
  }
  println!("cargo:rerun-if-changed={}", proto_dir);
  Ok(())
}
