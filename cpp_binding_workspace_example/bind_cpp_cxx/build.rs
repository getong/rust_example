fn main() {
  // cxx_build::bridge 解析 src/main.rs 里的 #[cxx::bridge] 模块，
  // 生成 C++ 侧胶水代码（含 rust/cxx.h），再和我们的 api.cpp 一起编译。
  // bridge 中返回 Result 的函数会把 C++ 异常转换成 Rust 侧的 Err。
  cxx_build::bridge("src/main.rs")
    .file("cpp/api.cpp")
    .include("cpp")
    .std("c++20")
    .compile("bind_cpp_cxx");

  for path in [
    "src/main.rs",
    "cpp/api.cpp",
    "cpp/api.h",
    "cpp/toml.hpp",
  ] {
    println!("cargo:rerun-if-changed={path}");
  }
}
