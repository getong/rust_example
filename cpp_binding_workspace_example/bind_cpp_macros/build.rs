fn main() {
  cpp_build::Config::new()
    .flag_if_supported("-std=c++17")
    .flag_if_supported("/std:c++17")
    .define("TOML_EXCEPTIONS", Some("0"))
    .build("src/main.rs");
}
