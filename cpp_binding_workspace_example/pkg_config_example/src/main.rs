// pkg-config 的作用：
//
// pkg-config 是类 Unix 系统上管理 C/C++ 库编译/链接参数的标准工具。
// 每个库安装后会带一个 .pc 文件（如 zlib.pc），里面记录了：
//   - 头文件搜索路径（对应 gcc 的 -I 参数）
//   - 库文件搜索路径（对应 -L 参数）
//   - 需要链接的库名（对应 -l 参数，如 -lz）
//   - 版本号、依赖的其他库
//
// 命令行验证：pkg-config --cflags --libs zlib
//
// Rust 的 pkg-config crate 就是它的封装。probe() 会：
//   1. 调用系统的 pkg-config 查找指定的库
//   2. 校验版本是否满足要求
//   3. 返回 Library 结构体（包含路径、库名、版本等信息）
//   4. 如果在 build.rs 中运行，还会自动向 cargo 输出 cargo:rustc-link-lib=z /
//      cargo:rustc-link-search=... 等指令， 让 Rust 程序能链接到这个 C 库
//
// 注意：pkg-config crate 的正规用法是写在 build.rs（构建脚本）里，
// 供 FFI 绑定链接原生库使用。这里放在 main.rs 只是为了直观演示 probe 的效果。

fn main() {
  // 原来的 "foo" 是一个不存在的占位库名，probe 必然失败。
  // 换成真实存在的 zlib（几乎所有系统都装有的压缩库）。
  let zlib = pkg_config::Config::new()
    .atleast_version("1.2.0")
    .probe("zlib")
    .expect("找不到 zlib，请确认已安装 pkg-config 和 zlib 开发包");

  println!("库名: {:?}", zlib.libs); // 链接的库名，即 -lz
  println!("版本: {}", zlib.version);
  println!("头文件路径 (-I): {:?}", zlib.include_paths);
  println!("库文件路径 (-L): {:?}", zlib.link_paths);

  // 再试一个例子：sqlite3
  match pkg_config::Config::new()
    .atleast_version("3.0")
    .probe("sqlite3")
  {
    Ok(lib) => println!("\nsqlite3 版本: {}, 库名: {:?}", lib.version, lib.libs),
    Err(e) => println!("\n未找到 sqlite3: {e}"),
  }
}
