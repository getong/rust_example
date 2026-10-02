use std::{env, path::Path};

use encrypt_file::{Result, encrypt_file};

fn main() -> Result<()> {
  let args: Vec<_> = env::args_os().skip(1).collect();
  if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
    println!("用法：encrypt <公钥文件> <输入文件> <加密文件>");
    return Ok(());
  }
  if args.len() != 3 {
    return Err("用法：encrypt <公钥文件> <输入文件> <加密文件>".into());
  }
  encrypt_file(
    Path::new(&args[0]),
    Path::new(&args[1]),
    Path::new(&args[2]),
  )?;
  println!("加密成功：{}", Path::new(&args[2]).display());
  Ok(())
}
