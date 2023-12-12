fn main() {
  let mut config = prost_build::Config::new();
  config
    .out_dir("src")
    .compile_protos(&["src/echo_message.proto"], &["."])
    .unwrap();
}
