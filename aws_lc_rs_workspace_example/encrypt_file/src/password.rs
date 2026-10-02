use std::path::Path;

use zeroize::Zeroizing;

use crate::{Error, Result, keys::KeyKind};

/// Supplied by the caller; the library never reads stdin or touches a terminal.
pub trait PasswordSource {
  fn password(&mut self, path: &Path, kind: KeyKind) -> Result<Zeroizing<String>>;
}

pub struct FixedPassword(pub Zeroizing<String>);
impl PasswordSource for FixedPassword {
  fn password(&mut self, _: &Path, _: KeyKind) -> Result<Zeroizing<String>> {
    Ok(Zeroizing::new(self.0.to_string()))
  }
}

pub struct NoPassword;
impl PasswordSource for NoPassword {
  fn password(&mut self, _: &Path, _: KeyKind) -> Result<Zeroizing<String>> {
    Err(Error::PasswordRequired)
  }
}

pub(crate) fn validate_password(password: &str) -> Result<()> {
  if password.is_empty() {
    return Err(Error::PasswordRequired);
  }
  if password.len() > 1024 {
    return Err(Error::PasswordTooLong);
  }
  Ok(())
}
