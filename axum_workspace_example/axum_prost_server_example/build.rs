use std::process::Command;

fn main() {
  let mut config = prost_build::Config::new();
  match config
    .out_dir("src")
    .compile_protos(&["todo.proto"], &["."])
  {
    Ok(()) => {
      if let Err(_) = Command::new("rustfmt").args(&["src/myapp.rs"]).status() {
        println!("cargo:warning=Failed to format generated protobuf files");
      }
    }
    err => println!("cargo:warning={:?}", err),
  }
}
