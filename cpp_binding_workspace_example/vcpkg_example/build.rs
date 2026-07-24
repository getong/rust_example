fn main() {
  // 从 vcpkg 中查找 tbb:得到头文件路径,并输出 cargo:rustc-link-* 链接指令
  let tbb = vcpkg::find_package("tbb").unwrap();

  // 编译调用 TBB 的 C++ shim,include 路径来自 vcpkg
  let mut build = cc::Build::new();
  build.cpp(true).std("c++23").file("src/tbb_demo.cpp");
  for include in &tbb.include_paths {
    build.include(include);
  }
  build.compile("tbb_demo");

  println!("cargo:rerun-if-changed=src/tbb_demo.cpp");
}
