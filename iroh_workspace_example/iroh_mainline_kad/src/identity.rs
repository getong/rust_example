use mainline::{Id, MutableItem, SigningKey};
use n0_error::Result;

use crate::parsing::parse_secret_key;

const DEFAULT_CLUSTER_KEY_BYTES: [u8; 32] = [42; 32];
const DEFAULT_CLUSTER_SALT: &[u8] = b"iroh-mainline-kad/v0";

#[derive(Debug, Clone)]
pub struct ClusterIdentity {
  signer: SigningKey,
  salt: Vec<u8>,
}

impl ClusterIdentity {
  pub fn from_secret_hex(secret_hex: Option<&str>, salt: impl Into<Vec<u8>>) -> Result<Self> {
    let bytes = match secret_hex {
      Some(value) => parse_secret_key(value)?,
      None => DEFAULT_CLUSTER_KEY_BYTES,
    };

    Ok(Self {
      signer: SigningKey::from_bytes(&bytes),
      salt: salt.into(),
    })
  }

  pub fn public_key(&self) -> [u8; 32] {
    self.signer.verifying_key().to_bytes()
  }

  pub fn salt(&self) -> &[u8] {
    &self.salt
  }

  pub fn target(&self) -> Id {
    MutableItem::target_from_key(&self.public_key(), Some(&self.salt))
  }

  pub(crate) fn signer(&self) -> SigningKey {
    self.signer.clone()
  }
}

impl Default for ClusterIdentity {
  fn default() -> Self {
    Self {
      signer: SigningKey::from_bytes(&DEFAULT_CLUSTER_KEY_BYTES),
      salt: DEFAULT_CLUSTER_SALT.to_vec(),
    }
  }
}

pub fn default_cluster_salt() -> Vec<u8> {
  DEFAULT_CLUSTER_SALT.to_vec()
}
