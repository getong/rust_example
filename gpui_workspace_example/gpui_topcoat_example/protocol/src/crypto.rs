//! Versioned, single-use post-quantum request/response channel.
//! Fresh client KEM keys per request; ephemeral ML-DSA server identity. No plaintext fallback.
use aws_lc_rs::{
  aead::{self, Aad, LessSafeKey, Nonce, UnboundKey},
  digest::{SHA256, digest},
  hkdf::{HKDF_SHA256, Salt},
  kem::{Ciphertext, DecapsulationKey, EncapsulationKey, ML_KEM_1024},
  rand::{SecureRandom, SystemRandom},
  signature::{KeyPair, ML_DSA_87, ML_DSA_87_SIGNING, PqdsaKeyPair, UnparsedPublicKey},
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

pub const SUITE: &str = "topcoat-pq-v2:ML-KEM-1024:ML-DSA-87:HKDF-SHA256:AES-256-GCM";
pub const HANDSHAKE_PATH: &str = "/pq/handshake";
pub const EXCHANGE_PATH: &str = "/pq/exchange";
pub const MAX_PLAINTEXT: usize = 4 * 1024 * 1024;
pub const MAX_WIRE: usize = MAX_PLAINTEXT * 2 + 4096;
pub type Result<T> = std::result::Result<T, CryptoError>;
#[derive(Debug)]
pub struct CryptoError;
impl std::fmt::Display for CryptoError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str("Invalid or unauthenticated encrypted message")
  }
}
impl std::error::Error for CryptoError {}
fn checked<T, E>(r: std::result::Result<T, E>) -> Result<T> {
  r.map_err(|_| CryptoError)
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ClientHello {
  pub suite: String,
  pub public_key: String,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ServerHello {
  pub public_key: String,
  pub suite: String,
  pub session: String,
  pub kem: String,
  pub signature: String,
}
#[derive(Serialize, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
  pub session: String,
  pub ciphertext: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiRequest {
  pub method: String,
  pub path: String,
  pub body: String,
  pub form: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiResponse {
  pub status: u16,
  pub body: String,
}

pub fn encode(bytes: &[u8]) -> String {
  hex::encode(bytes)
}
pub fn decode(value: &str, max: usize) -> Result<Vec<u8>> {
  if value.len() > max * 2
    || !value
      .bytes()
      .all(|b| b.is_ascii_digit() || (b'a' ..= b'f').contains(&b))
  {
    return Err(CryptoError);
  }
  checked(hex::decode(value))
}
fn exact(value: &str, size: usize) -> Result<Vec<u8>> {
  let bytes = decode(value, size)?;
  if bytes.len() != size {
    return Err(CryptoError);
  }
  Ok(bytes)
}
fn transcript(public: &[u8], session: &[u8], kem: &[u8], signing: &[u8]) -> Vec<u8> {
  [SUITE.as_bytes(), public, session, kem, signing].concat()
}

pub struct Identity(PqdsaKeyPair);
impl Identity {
  pub fn generate() -> Result<Self> {
    Ok(Self(checked(PqdsaKeyPair::generate(&ML_DSA_87_SIGNING))?))
  }
  pub fn public_key(&self) -> String {
    encode(self.0.public_key().as_ref())
  }
  pub fn accept(&self, hello: &ClientHello) -> Result<(ServerHello, ServerChannel)> {
    if hello.suite != SUITE {
      return Err(CryptoError);
    }
    let public = exact(&hello.public_key, 1568)?;
    let key = checked(EncapsulationKey::new(&ML_KEM_1024, &public))?;
    let (kem, shared) = checked(key.encapsulate())?;
    let mut session = [0; 32];
    checked(SystemRandom::new().fill(&mut session))?;
    let transcript = transcript(
      &public,
      &session,
      kem.as_ref(),
      self.0.public_key().as_ref(),
    );
    let mut signature = vec![0; ML_DSA_87_SIGNING.signature_len()];
    checked(self.0.sign(&transcript, &mut signature))?;
    let keys = Keys::derive(shared.as_ref(), &transcript, encode(&session))?;
    Ok((
      ServerHello {
        public_key: self.public_key(),
        suite: SUITE.into(),
        session: encode(&session),
        kem: encode(kem.as_ref()),
        signature: encode(&signature),
      },
      ServerChannel(keys),
    ))
  }
}

pub struct ClientHandshake {
  private: DecapsulationKey,
  public: Vec<u8>,
}
impl ClientHandshake {
  pub fn begin() -> Result<(Self, ClientHello)> {
    let private = checked(DecapsulationKey::generate(&ML_KEM_1024))?;
    let public = checked(checked(private.encapsulation_key())?.key_bytes())?
      .as_ref()
      .to_vec();
    let hello = ClientHello {
      suite: SUITE.into(),
      public_key: encode(&public),
    };
    Ok((Self { private, public }, hello))
  }
  /// The supplied verification key must come from authenticated transport or an external trust
  /// anchor. A signature checked against an in-band ephemeral key alone does not establish server
  /// identity.
  pub fn finish(self, hello: &ServerHello, pinned_public_key: &str) -> Result<ClientChannel> {
    if hello.suite != SUITE {
      return Err(CryptoError);
    }
    let session = exact(&hello.session, 32)?;
    let kem = exact(&hello.kem, 1568)?;
    let signature = exact(&hello.signature, 4627)?;
    let public = exact(pinned_public_key.trim(), 2592)?;
    if hello.public_key != pinned_public_key.trim() {
      return Err(CryptoError);
    }
    let transcript = transcript(&self.public, &session, &kem, &public);
    checked(UnparsedPublicKey::new(&ML_DSA_87, public).verify(&transcript, &signature))?;
    let secret = checked(self.private.decapsulate(Ciphertext::from(kem.as_slice())))?;
    Ok(ClientChannel(Keys::derive(
      secret.as_ref(),
      &transcript,
      hello.session.clone(),
    )?))
  }
}
struct Keys {
  send: LessSafeKey,
  receive: LessSafeKey,
  session: String,
  hash: Vec<u8>,
}
impl Keys {
  fn derive(secret: &[u8], transcript: &[u8], session: String) -> Result<Self> {
    let hash = digest(&SHA256, transcript).as_ref().to_vec();
    let prk = Salt::new(HKDF_SHA256, &hash).extract(secret);
    let key = |label: &[u8]| -> Result<LessSafeKey> {
      let info = [label];
      let okm = checked(prk.expand(&info, &aead::AES_256_GCM))?;
      let mut bytes = Zeroizing::new([0; 32]);
      checked(okm.fill(bytes.as_mut()))?;
      Ok(LessSafeKey::new(checked(UnboundKey::new(
        &aead::AES_256_GCM,
        bytes.as_ref(),
      ))?))
    };
    Ok(Self {
      send: key(b"c2s")?,
      receive: key(b"s2c")?,
      session,
      hash,
    })
  }
  fn seal(&self, value: &[u8], response: bool) -> Result<Envelope> {
    if value.len() > MAX_PLAINTEXT {
      return Err(CryptoError);
    }
    let mut ciphertext = value.to_vec();
    let key = if response { &self.receive } else { &self.send };
    checked(key.seal_in_place_append_tag(
      Nonce::assume_unique_for_key([0; 12]),
      Aad::from(&self.hash),
      &mut ciphertext,
    ))?;
    Ok(Envelope {
      session: self.session.clone(),
      ciphertext: encode(&ciphertext),
    })
  }
  fn open(&self, envelope: &Envelope, response: bool) -> Result<Zeroizing<Vec<u8>>> {
    if envelope.session != self.session {
      return Err(CryptoError);
    }
    let mut bytes = Zeroizing::new(decode(&envelope.ciphertext, MAX_PLAINTEXT + 16)?);
    let key = if response { &self.receive } else { &self.send };
    let len = checked(key.open_in_place(
      Nonce::assume_unique_for_key([0; 12]),
      Aad::from(&self.hash),
      &mut bytes,
    ))?
    .len();
    bytes.truncate(len);
    Ok(bytes)
  }
}
// Ownership enforces exactly one encryption in each direction: fixed zero nonces
// are unique because every exchange derives fresh, direction-separated keys.
pub struct ClientChannel(Keys);
pub struct ResponseReader(Keys);
pub struct ServerChannel(Keys);
pub struct ResponseWriter(Keys);
impl ClientChannel {
  pub fn seal(self, value: &[u8]) -> Result<(Envelope, ResponseReader)> {
    Ok((self.0.seal(value, false)?, ResponseReader(self.0)))
  }
}
impl ResponseReader {
  pub fn open(self, value: &Envelope) -> Result<Zeroizing<Vec<u8>>> {
    self.0.open(value, true)
  }
}
impl ServerChannel {
  pub fn open(self, value: &Envelope) -> Result<(Zeroizing<Vec<u8>>, ResponseWriter)> {
    Ok((self.0.open(value, false)?, ResponseWriter(self.0)))
  }
}
impl ResponseWriter {
  pub fn seal(self, value: &[u8]) -> Result<Envelope> {
    self.0.seal(value, true)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn bidirectional_authenticated_exchange() {
    let identity = Identity::generate().unwrap();
    let (client, hello) = ClientHandshake::begin().unwrap();
    let (reply, server) = identity.accept(&hello).unwrap();
    let channel = client.finish(&reply, &identity.public_key()).unwrap();
    let (request, reader) = channel.seal(b"private request").unwrap();
    let (plain, writer) = server.open(&request).unwrap();
    assert_eq!(&**plain, b"private request");
    let response = writer.seal(b"private response").unwrap();
    assert_eq!(&**reader.open(&response).unwrap(), b"private response");
  }
  #[test]
  fn rejects_wrong_pin_stale_hello_suite_and_tampering() {
    let identity = Identity::generate().unwrap();
    for variant in 0 .. 5 {
      let (client, hello) = ClientHandshake::begin().unwrap();
      let (mut reply, _) = identity.accept(&hello).unwrap();
      let mut pin = identity.public_key();
      match variant {
        0 => pin = Identity::generate().unwrap().public_key(),
        1 => reply.suite.push('x'),
        2 => {
          let b = decode(&reply.session, 32).unwrap()[0] ^ 1;
          reply.session.replace_range(.. 2, &format!("{b:02x}"));
        }
        3 => {
          let b = decode(&reply.signature, 4627).unwrap()[0] ^ 1;
          reply.signature.replace_range(.. 2, &format!("{b:02x}"));
        }
        _ => {
          let (_, other) = ClientHandshake::begin().unwrap();
          reply = identity.accept(&other).unwrap().0;
        }
      }
      assert!(client.finish(&reply, &pin).is_err());
    }
    let (client, hello) = ClientHandshake::begin().unwrap();
    let (reply, server) = identity.accept(&hello).unwrap();
    let (mut envelope, reader) = client
      .finish(&reply, &identity.public_key())
      .unwrap()
      .seal(b"secret")
      .unwrap();
    // A request ciphertext cannot be reflected as a response.
    assert!(reader.open(&envelope).is_err());
    envelope.ciphertext.push_str("00");
    assert!(server.open(&envelope).is_err());
  }
}
