use std::{path::Path, process::Command};

/// 根据编译目标推导 vcpkg triplet。
fn vcpkg_triplet() -> &'static str {
  let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();
  let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();
  match (os.as_str(), arch.as_str()) {
    ("macos", "aarch64") => "arm64-osx",
    ("macos", "x86_64") => "x64-osx",
    ("linux", "aarch64") => "arm64-linux",
    ("linux", "x86_64") => "x64-linux",
    ("windows", _) => "x64-windows",
    other => panic!("no vcpkg triplet mapping for {other:?}"),
  }
}

fn main() {
  // C++ 依赖声明在本 crate 的 vcpkg.json（manifest 模式）里：
  // boost-asio >= 1.91.0。vcpkg install 会把头文件装到项目本地的
  // vcpkg_installed/<triplet>/ 下，不依赖系统 brew / 全局安装。
  let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
  let triplet = vcpkg_triplet();
  let include = Path::new(&manifest_dir)
    .join("vcpkg_installed")
    .join(triplet)
    .join("include");

  if !include.join("boost/asio.hpp").exists() {
    // 优先用 VCPKG_ROOT 里 bootstrap 出来的二进制：brew 装的 vcpkg
    // 可能和 VCPKG_ROOT 仓库版本不匹配（vcpkg-tools.json schema 冲突）
    let vcpkg_bin = std::env::var("VCPKG_ROOT")
      .ok()
      .map(|root| Path::new(&root).join("vcpkg"))
      .filter(|bin| bin.exists())
      .unwrap_or_else(|| "vcpkg".into());
    let status = Command::new(vcpkg_bin)
      .args(["install", "--triplet", triplet])
      .current_dir(&manifest_dir)
      .status()
      .expect("failed to run `vcpkg install` (is vcpkg on PATH and VCPKG_ROOT set?)");
    assert!(status.success(), "`vcpkg install` failed");
    assert!(
      include.join("boost/asio.hpp").exists(),
      "boost/asio.hpp still missing after vcpkg install"
    );
  }

  // Boost.Asio 纯头文件用法（Boost >= 1.69 后 Boost.System 也是 header-only），
  // 只需要 include 路径，不需要链接任何 boost 库。
  cxx_build::bridge("src/main.rs")
    .file("src/api.cpp")
    .std("c++20")
    .include(include)
    // bridge 里 include!("src/api.h") 是相对 crate 根的路径，
    // 把 crate 根加进搜索路径让它能被解析
    .include(&manifest_dir)
    .compile("tokio_call_asio");

  println!("cargo:rerun-if-changed=src/main.rs");
  println!("cargo:rerun-if-changed=src/api.cpp");
  println!("cargo:rerun-if-changed=src/api.h");
  println!("cargo:rerun-if-changed=vcpkg.json");
}
