fn main() {
  // cxx_build::bridge 解析 src/main.rs 里的 #[cxx::bridge] 模块，
  // 生成 C++ 侧胶水代码（含 rust/cxx.h），再和我们的 api.cpp 一起编译。
  // 与 bind_cpp_manual 不同，这里保留 toml++ 默认的异常模式：
  // C++ 抛出的 std::exception 会被 cxx 转成 Rust 侧的 Err。
  cxx_build::bridge("src/main.rs")
    .file("src/api.cpp")
    .std("c++20")
    .compile("bind_cpp_cxx");

  println!("cargo:rerun-if-changed=src/main.rs");
  println!("cargo:rerun-if-changed=src/api.cpp");
  println!("cargo:rerun-if-changed=include/api.h");
}
