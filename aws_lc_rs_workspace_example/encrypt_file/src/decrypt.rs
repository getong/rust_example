use std::{env, path::Path};

use encrypt_file::{Result, decrypt_file, generate_keys};

const USAGE: &str =
  "用法：decrypt <私钥文件> <加密文件> <输出文件>\n      decrypt --keygen <公钥文件> <私钥文件>";

fn main() -> Result<()> {
  let args: Vec<_> = env::args_os().skip(1).collect();
  if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
    println!("{USAGE}");
    return Ok(());
  }
  if args.len() != 3 {
    return Err(USAGE.into());
  }
  if args[0] == "--keygen" {
    generate_keys(Path::new(&args[1]), Path::new(&args[2]))?;
    println!("密钥已生成。公钥用于加密，私钥请独立妥善保存。");
  } else {
    decrypt_file(
      Path::new(&args[0]),
      Path::new(&args[1]),
      Path::new(&args[2]),
    )?;
    println!("解密成功：{}", Path::new(&args[2]).display());
  }
  Ok(())
}
