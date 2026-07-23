fn main() {
  cc::Build::new()
    .cpp(true)
    .std("c++20")
    .define("TOML_EXCEPTIONS", "0")
    .file("api.cpp")
    .compile("api");
}
