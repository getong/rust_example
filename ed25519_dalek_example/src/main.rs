use ed25519_dalek::{Signature, Signer, SigningKey};
use rand::rngs::OsRng;

fn main() {
  let mut csprng = OsRng;
  let signing_key: SigningKey = SigningKey::generate(&mut csprng);

  let message: &[u8] = b"This is a test of the tsunami alert system.";
  let signature: Signature = signing_key.sign(message);

  assert!(signing_key.verify(message, &signature).is_ok());
}
