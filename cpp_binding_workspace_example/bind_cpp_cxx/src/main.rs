use cxx::UniquePtr;

// #[cxx::bridge] 是声明式的双向桥：这里写下的签名会由 cxx 在编译期
// 与 cpp/api.h 里的 C++ 声明逐一核对，两侧不一致直接编译失败——
// 这正是它优于手抄 extern "C" 声明的地方（抄错签名 = UB，而这里是编译错误）。
#[cxx::bridge]
mod ffi {
  unsafe extern "C++" {
    include!("api.h");

    /// 不透明的 C++ 类型：Rust 侧只能通过引用或 UniquePtr 使用它。
    type TomlTable;

    /// C++ 抛出的异常被 cxx 自动转成 Err(cxx::Exception)。
    fn parse_file(path: &str) -> Result<UniquePtr<TomlTable>>;
    fn to_toml_string(table: &TomlTable) -> String;
    fn contains_key(table: &TomlTable, key: &str) -> bool;
    fn get_string(table: &TomlTable, dotted_path: &str) -> Result<String>;
  }
}

fn main() {
  // 解析本 crate 自己的 Cargo.toml，路径不受运行目录影响
  let path = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");

  // 注意调用处没有一个 unsafe 块：cxx 生成的接口层是 safe 的，
  // &str/String/UniquePtr 的跨语言转换都由 cxx 保证。
  let table: UniquePtr<ffi::TomlTable> = match ffi::parse_file(path) {
    Ok(table) => table,
    Err(err) => {
      eprintln!("parse failed: {err}");
      return;
    }
  };
  // UniquePtr 离开作用域时自动调用 C++ 侧析构函数，无需手写 free

  let table = table.as_ref().expect("non-null table");

  println!(
    "contains [package]: {}",
    ffi::contains_key(table, "package")
  );
  println!(
    "contains [workspace]: {}",
    ffi::contains_key(table, "workspace")
  );

  match ffi::get_string(table, "package.name") {
    Ok(name) => println!("package.name = {name}"),
    Err(err) => println!("get_string failed: {err}"),
  }
  match ffi::get_string(table, "package.no_such_key") {
    Ok(v) => println!("unexpected: {v}"),
    Err(err) => println!("expected error: {err}"),
  }

  println!("--- full table ---");
  println!("{}", ffi::to_toml_string(table));

  // 演示 C++ 异常 -> Rust Err 的错误路径
  if let Err(err) = ffi::parse_file("no_such_file.toml") {
    println!("expected parse error: {err}");
  }
}
