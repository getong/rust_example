use std::{env, path::Path, process::ExitCode};

use encrypt_file::{Error, Result, encrypt_file};
mod cli;
const USAGE: &str = "用法：encrypt --sign-key <发送方签名私钥> <接收方公钥> <输入文件> <加密文件>";
fn run() -> Result<()> {
  let args: Vec<_> = env::args_os().skip(1).collect();
  if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
    println!("{USAGE}");
    return Ok(());
  }
  if args.len() != 5 || args[0] != "--sign-key" {
    return Err(Error::Usage(USAGE));
  }
  encrypt_file(
    Path::new(&args[2]),
    Path::new(&args[1]),
    Path::new(&args[3]),
    Path::new(&args[4]),
    &mut cli::CliPasswords,
  )?;
  println!("加密并签名成功：{}", Path::new(&args[4]).display());
  Ok(())
}
fn main() -> ExitCode {
  cli::finish(run())
}
