use std::{fs::create_dir_all, process::Command};

use walkdir::WalkDir;

fn main() {
  let protobuf_directory = "messages";
  let output_directory = "src/protos";
  _ = create_dir_all(output_directory);

  let proto_files: Vec<_> = WalkDir::new(protobuf_directory)
    .into_iter()
    .filter_map(|entry| entry.ok())
    .filter(|entry| entry.path().is_file() && entry.path().extension() == Some("proto".as_ref()))
    .filter_map(|entry| entry.path().to_str().map(String::from))
    .collect();
  println!("proto_files: {:?}", proto_files);

  match prost_build::Config::new()
    .out_dir(output_directory)
    .include_file("mod.rs")
    .enable_type_names()
    .type_attribute("ReadRequest", "#[allow(dead_code)]")
    .type_attribute("ReadResponse", "#[allow(dead_code)]")
    .type_attribute("SampleSchema", "#[allow(dead_code)]")
    .type_attribute("StateSignal", "#[allow(dead_code)]")
    .type_attribute("OtherMessage", "#[allow(dead_code)]")
    .compile_protos(&proto_files, &["."])
  {
    Ok(()) => {
      let generated_proto_files: Vec<_> = WalkDir::new(output_directory)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file() && entry.path().extension() == Some("rs".as_ref()))
        .filter_map(|entry| entry.path().to_str().map(String::from))
        .collect();
      if let Err(_) = Command::new("rustfmt")
        .args(&generated_proto_files)
        .status()
      {
        println!("cargo:warning=Failed to format generated protobuf files");
      }
    }
    err => println!("cargo:warning={:?}", err),
  }
}
