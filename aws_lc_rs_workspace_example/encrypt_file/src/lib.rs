//! Versioned file encryption with explicit password sources and pinned sender verification.
macro_rules! measure {
  ($name:literal, $expr:expr) => {{
    let _span = crate::perf::span($name);
    $expr
  }};
}

pub mod crypto;
mod error;
pub mod file;
pub mod format;
pub mod kdf;
pub mod keys;
pub mod password;
mod perf;
mod platform;
pub mod signing;

pub use crypto::{EncryptedFile, decrypt_bytes, decrypt_file, encrypt_bytes, encrypt_file};
pub use error::{Error, Result};
pub use format::*;
pub use keys::{KeyKind, Protection, generate_keys, generate_signing_keys, read_key};
pub use password::{FixedPassword, NoPassword, PasswordSource};
pub use signing::Verification;
