use std::{
  io::{BufRead, IsTerminal, Read, Write, stderr, stdin},
  path::Path,
  process::ExitCode,
};

use encrypt_file::{Error, KeyKind, PasswordSource, Result};
use inquire::{Password, PasswordDisplayMode};
use zeroize::Zeroizing;

pub struct CliPasswords;
impl PasswordSource for CliPasswords {
  fn password(&mut self, path: &Path, _: KeyKind) -> Result<Zeroizing<String>> {
    read_password(&format!("请输入密钥密码（{}）：", path.display()))
  }
}

pub fn read_password(prompt: &str) -> Result<Zeroizing<String>> {
  let password = if stdin().is_terminal() {
    Zeroizing::new(
      Password::new(prompt)
        .with_display_mode(PasswordDisplayMode::Masked)
        .without_confirmation()
        .prompt()
        .map_err(|e| Error::Input(e.to_string()))?,
    )
  } else {
    let mut stderr = stderr().lock();
    write!(stderr, "{prompt}")?;
    stderr.flush()?;
    // Room for the full 1024-byte password, CRLF, and an overflow sentinel.
    let mut password = Zeroizing::new(String::with_capacity(1027));
    if stdin().lock().take(1027).read_line(&mut password)? == 0 {
      return Err(Error::InputCancelled);
    }
    if password.ends_with('\n') {
      password.pop();
      if password.ends_with('\r') {
        password.pop();
      }
    }
    password
  };
  if password.len() > 1024 {
    return Err(Error::PasswordTooLong);
  }
  Ok(password)
}

pub fn finish(result: Result<()>) -> ExitCode {
  match result {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("错误：{error}");
      ExitCode::FAILURE
    }
  }
}
