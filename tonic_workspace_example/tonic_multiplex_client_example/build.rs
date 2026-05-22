use std::{env, path::PathBuf};

fn main() {
  let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
  tonic_prost_build::configure()
    .file_descriptor_set_path(out_dir.join("helloworld_descriptor.bin"))
    .compile_protos(&["proto/helloworld/helloworld.proto"], &["proto"])
    .unwrap();

  tonic_prost_build::compile_protos("proto/unaryecho/echo.proto").unwrap();
}
