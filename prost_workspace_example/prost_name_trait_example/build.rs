use std::process::Command;

fn main() {
  let mut config = prost_build::Config::new();
  config.type_attribute("OtherMessage", "#[allow(dead_code)]");
  match config
    .out_dir("src")
    .enable_type_names()
    .compile_protos(&["src/message.proto"], &["."])
  {
    Ok(()) => {
      if let Err(_) = Command::new("rustfmt").args(&["src/mypackage.rs"]).status() {
        println!("cargo:warning=Failed to format generated protobuf files");
      }
    }
    err => println!("cargo:warning={:?}", err),
  }
}
