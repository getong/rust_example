fn main() {
  cpp_build::Config::new()
    .flag_if_supported("-std=c++20")
    .flag_if_supported("/std:c++20")
    .define("TOML_EXCEPTIONS", Some("0"))
    .build("src/main.rs");
}
