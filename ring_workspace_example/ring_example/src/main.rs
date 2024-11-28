// copy from https://briansmith.org/rustdoc/ring/pbkdf2/index.html
use std::{collections::HashMap, num::NonZeroU32};

use ring::{digest, pbkdf2};

static PBKDF2_ALG: pbkdf2::Algorithm = pbkdf2::PBKDF2_HMAC_SHA256;
const CREDENTIAL_LEN: usize = digest::SHA256_OUTPUT_LEN;
pub type Credential = [u8; CREDENTIAL_LEN];

enum Error {
  WrongUsernameOrPassword,
}

struct PasswordDatabase {
  pbkdf2_iterations: NonZeroU32,
  db_salt_component: [u8; 16],

  // Normally this would be a persistent database.
  storage: HashMap<String, Credential>,
}

impl PasswordDatabase {
  pub fn store_password(&mut self, username: &str, password: &str) {
    let salt = self.salt(username);
    let mut to_store: Credential = [0u8; CREDENTIAL_LEN];
    pbkdf2::derive(
      PBKDF2_ALG,
      self.pbkdf2_iterations,
      &salt,
      password.as_bytes(),
      &mut to_store,
    );
    self.storage.insert(String::from(username), to_store);
  }

  pub fn verify_password(&self, username: &str, attempted_password: &str) -> Result<(), Error> {
    match self.storage.get(username) {
      Some(actual_password) => {
        let salt = self.salt(username);
        pbkdf2::verify(
          PBKDF2_ALG,
          self.pbkdf2_iterations,
          &salt,
          attempted_password.as_bytes(),
          actual_password,
        )
        .map_err(|_| Error::WrongUsernameOrPassword)
      }

      None => Err(Error::WrongUsernameOrPassword),
    }
  }

  // The salt should have a user-specific component so that an attacker
  // cannot crack one password for multiple users in the database. It
  // should have a database-unique component so that an attacker cannot
  // crack the same user's password across databases in the unfortunate
  // but common case that the user has used the same password for
  // multiple systems.
  fn salt(&self, username: &str) -> Vec<u8> {
    let mut salt = Vec::with_capacity(self.db_salt_component.len() + username.as_bytes().len());
    salt.extend(self.db_salt_component.as_ref());
    salt.extend(username.as_bytes());
    salt
  }
}

fn main() {
  // Normally these parameters would be loaded from a configuration file.
  let mut db = PasswordDatabase {
    pbkdf2_iterations: NonZeroU32::new(100_000).unwrap(),
    db_salt_component: [
      // This value was generated from a secure PRNG.
      0xd6, 0x26, 0x98, 0xda, 0xf4, 0xdc, 0x50, 0x52, 0x24, 0xf2, 0x27, 0xd1, 0xfe, 0x39, 0x01, 0x8a,
    ],
    storage: HashMap::new(),
  };

  db.store_password("alice", "@74d7]404j|W}6u");

  // An attempt to log in with the wrong password fails.
  assert!(db.verify_password("alice", "wrong password").is_err());

  // Normally there should be an expoentially-increasing delay between
  // attempts to further protect against online attacks.

  // An attempt to log in with the right password succeeds.
  assert!(db.verify_password("alice", "@74d7]404j|W}6u").is_ok());
}
