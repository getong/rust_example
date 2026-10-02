use std::{env, path::Path, process::ExitCode};

use encrypt_file::{
  Error, KeyKind, Protection, Result, Verification, decrypt_file, generate_keys,
  generate_signing_keys, keys::fingerprint_hex, read_key,
};
mod cli;
const USAGE: &str =
  "用法：decrypt --verify-key <可信发送方公钥> <接收方私钥> <加密文件> <输出文件>\n      decrypt \
   --allow-unsigned-legacy <私钥> <旧v1密文> <输出>\n      decrypt --keygen [--no-password] \
   [--protect-public] <公钥> <私钥>\n      decrypt --sign-keygen [--no-password] <签名公钥> \
   <签名私钥>\n      decrypt --fingerprint <kem|sign> <公钥文件>";
fn run() -> Result<()> {
  let args: Vec<_> = env::args_os().skip(1).collect();
  if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
    println!("{USAGE}");
    return Ok(());
  }
  let Some(command) = args.first() else {
    return Err(Error::Usage(USAGE));
  };
  let mut passwords = cli::CliPasswords;
  if command == "--keygen" || command == "--sign-keygen" {
    let signing = command == "--sign-keygen";
    let mut no_password = false;
    let mut protect_public = false;
    let mut public_option_seen = false;
    let mut index = 1;
    while index < args.len() {
      if args[index] == "--no-password" && !no_password {
        no_password = true;
      } else if args[index] == "--protect-public" && !signing && !public_option_seen {
        protect_public = true;
        public_option_seen = true;
      } else if args[index] == "--public-unprotected" && !signing && !public_option_seen {
        // Retain the old explicit option as a compatible alias for the new default.
        protect_public = false;
        public_option_seen = true;
      } else {
        break;
      }
      index += 1;
    }
    if args.len() != index + 2
      || (no_password && protect_public)
      || args[index].to_string_lossy().starts_with("--")
    {
      return Err(Error::Usage(USAGE));
    }
    let public = Path::new(&args[index]);
    let private = Path::new(&args[index + 1]);
    if public == private || public.try_exists()? || private.try_exists()? {
      return Err(Error::OutputExists);
    }
    let password;
    let protection = if no_password {
      eprintln!("警告：--no-password 将以未加密形式保存私钥。");
      Protection::Unprotected
    } else {
      password = new_password()?;
      Protection::Password(&password)
    };
    let fingerprint = if signing {
      generate_signing_keys(public, private, protection)?
    } else {
      generate_keys(public, private, protection, protect_public)?
    };
    println!("密钥已生成。公钥 SHA-256 指纹：{fingerprint}");
  } else if command == "--fingerprint" && args.len() == 3 {
    let kind = if args[1] == "kem" {
      KeyKind::KemPublic
    } else if args[1] == "sign" {
      KeyKind::SignPublic
    } else {
      return Err(Error::Usage(USAGE));
    };
    let bytes = read_key(Path::new(&args[2]), kind, &mut passwords)?;
    println!("SHA-256 {}", fingerprint_hex(&bytes));
  } else if command == "--verify-key" && args.len() == 5 {
    let verifier = read_key(Path::new(&args[1]), KeyKind::SignPublic, &mut passwords)?;
    decrypt_file(
      Path::new(&args[2]),
      Path::new(&args[3]),
      Path::new(&args[4]),
      Verification::Trusted(&verifier),
      &mut passwords,
    )?;
    println!("签名验证及解密成功：{}", Path::new(&args[4]).display());
  } else if command == "--allow-unsigned-legacy" && args.len() == 4 {
    eprintln!("警告：旧 v1 文件没有发送方签名，无法确认来源。");
    decrypt_file(
      Path::new(&args[1]),
      Path::new(&args[2]),
      Path::new(&args[3]),
      Verification::AllowUnsignedLegacy,
      &mut passwords,
    )?;
    println!("旧文件解密成功：{}", Path::new(&args[3]).display());
  } else {
    return Err(Error::Usage(USAGE));
  }
  Ok(())
}
fn main() -> ExitCode {
  cli::finish(run())
}

fn new_password() -> Result<zeroize::Zeroizing<String>> {
  let password = cli::read_password("请设置密钥密码：")?;
  if password.is_empty() {
    return Err(Error::PasswordRequired);
  }
  if password != cli::read_password("请再次输入密钥密码：")? {
    return Err(Error::PasswordMismatch);
  }
  Ok(password)
}
