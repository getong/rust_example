fn main() {
  let path = std::path::PathBuf::from("src");
  let mut b = autocxx_build::Builder::new("src/main.rs", [&path])
    .build()
    .expect("Failed to build autocxx");
  b.flag_if_supported("-std=c++14").compile("autocxx-demo");

  println!("cargo:rerun-if-changed=src/main.rs");
  println!("cargo:rerun-if-changed=src/input.h");
}
