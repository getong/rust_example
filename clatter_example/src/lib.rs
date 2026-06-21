#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_code)]

//! Small, fixed-buffer Clatter example that remains usable from `no_std` code.
//!
//! The library code is generic over the random source. On embedded or bare-metal
//! targets, provide a hardware RNG type that implements [`clatter::traits::Rng`].
//! The binary target enables the `system-rng` feature and uses Clatter's
//! `getrandom` backed RNG only for local demonstration.

use core::fmt;

use clatter::{
  HybridHandshakeCore, HybridHandshakeParams,
  crypto::{cipher::AesGcm, dh::X25519, hash::Sha512, kem::rust_crypto_ml_kem::MlKem512},
  error::{HandshakeError, TransportError},
  handshakepattern::noise_hybrid_nn,
  traits::{Handshaker, Rng},
};

/// Domain separation bytes mixed into the Noise handshake hash.
pub const PROLOGUE: &[u8] = b"clatter-example/noise-hybrid-nn/v1";

/// Plaintext sent by the initiator after the handshake enters transport mode.
pub const INITIATOR_TRANSPORT_MESSAGE: &[u8] = b"hello from initiator over hybrid Noise";

/// Plaintext sent by the responder after the handshake enters transport mode.
pub const RESPONDER_TRANSPORT_MESSAGE: &[u8] = b"hello from responder over hybrid Noise";

/// Stack buffer size used for handshake and transport messages.
pub const MESSAGE_BUFFER_LEN: usize = 4096;

/// Maximum plaintext copied into the returned demo report.
pub const MAX_TRANSPORT_PAYLOAD_LEN: usize = 96;

type DemoDh = X25519;
type DemoKem = MlKem512;
type DemoCipher = AesGcm;
type DemoHash = Sha512;
type DemoHandshake<RNG> = HybridHandshakeCore<DemoDh, DemoKem, DemoKem, DemoCipher, DemoHash, RNG>;
type DemoParams<'a> = HybridHandshakeParams<'a, DemoDh, DemoKem, DemoKem>;

/// Error returned by the fixed-buffer demo exchange.
#[derive(Debug)]
pub enum DemoError {
  /// Clatter rejected or failed a handshake operation.
  Handshake(HandshakeError),
  /// Clatter rejected or failed a transport operation.
  Transport(TransportError),
  /// The peers did not both reach the transport state.
  HandshakeIncomplete,
  /// A plaintext was larger than [`MAX_TRANSPORT_PAYLOAD_LEN`].
  PlaintextTooLarge,
}

impl fmt::Display for DemoError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Handshake(error) => write!(f, "handshake failed: {error}"),
      Self::Transport(error) => write!(f, "transport failed: {error}"),
      Self::HandshakeIncomplete => write!(f, "handshake did not complete"),
      Self::PlaintextTooLarge => write!(f, "transport plaintext too large"),
    }
  }
}

#[cfg(feature = "std")]
impl std::error::Error for DemoError {}

impl From<HandshakeError> for DemoError {
  fn from(error: HandshakeError) -> Self {
    Self::Handshake(error)
  }
}

impl From<TransportError> for DemoError {
  fn from(error: TransportError) -> Self {
    Self::Transport(error)
  }
}

/// Result of one complete hybrid Noise handshake and encrypted round trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoOutcome {
  /// Initiator-to-responder handshake message length.
  pub first_handshake_message_len: usize,
  /// Responder-to-initiator handshake message length.
  pub second_handshake_message_len: usize,
  /// Initiator-to-responder encrypted transport frame length.
  pub alice_to_bob_ciphertext_len: usize,
  /// Responder-to-initiator encrypted transport frame length.
  pub bob_to_alice_ciphertext_len: usize,
  bob_plaintext: [u8; MAX_TRANSPORT_PAYLOAD_LEN],
  bob_plaintext_len: usize,
  alice_plaintext: [u8; MAX_TRANSPORT_PAYLOAD_LEN],
  alice_plaintext_len: usize,
}

impl DemoOutcome {
  /// Plaintext decrypted by Bob from Alice's transport message.
  #[must_use]
  pub fn bob_plaintext(&self) -> &[u8] {
    &self.bob_plaintext[.. self.bob_plaintext_len]
  }

  /// Plaintext decrypted by Alice from Bob's transport message.
  #[must_use]
  pub fn alice_plaintext(&self) -> &[u8] {
    &self.alice_plaintext[.. self.alice_plaintext_len]
  }
}

/// Run a full `Noise_hybridNN_X25519+MLKEM512_AESGCM_SHA512` exchange.
///
/// This performs a two-message hybrid Noise handshake, finalizes both peers into
/// transport mode, then sends one authenticated encrypted message in each
/// direction. All protocol buffers are fixed-size stack arrays, and no heap API
/// is used by this crate.
///
/// # Errors
///
/// Returns [`DemoError`] if the Clatter handshake/transport state machines
/// reject an operation or a fixed output buffer is too small.
pub fn run_hybrid_exchange_with_rng<RNG>() -> Result<DemoOutcome, DemoError>
where
  RNG: Rng,
{
  let alice_params = DemoParams::new(noise_hybrid_nn(), true).with_prologue(PROLOGUE);
  let bob_params = DemoParams::new(noise_hybrid_nn(), false).with_prologue(PROLOGUE);

  let mut alice = DemoHandshake::<RNG>::new(alice_params)?;
  let mut bob = DemoHandshake::<RNG>::new(bob_params)?;

  let mut alice_buf = [0_u8; MESSAGE_BUFFER_LEN];
  let mut bob_buf = [0_u8; MESSAGE_BUFFER_LEN];

  let first_handshake_message_len = alice.write_message(&[], &mut alice_buf)?;
  bob.read_message(&alice_buf[.. first_handshake_message_len], &mut bob_buf)?;

  let second_handshake_message_len = bob.write_message(&[], &mut bob_buf)?;
  alice.read_message(&bob_buf[.. second_handshake_message_len], &mut alice_buf)?;

  if !alice.is_finished() || !bob.is_finished() {
    return Err(DemoError::HandshakeIncomplete);
  }

  let mut alice = alice.finalize()?;
  let mut bob = bob.finalize()?;

  let alice_to_bob_ciphertext_len = alice.send(INITIATOR_TRANSPORT_MESSAGE, &mut alice_buf)?;
  let bob_plaintext_len = bob.receive(&alice_buf[.. alice_to_bob_ciphertext_len], &mut bob_buf)?;
  let bob_plaintext = copy_plaintext(&bob_buf[.. bob_plaintext_len])?;

  let bob_to_alice_ciphertext_len = bob.send(RESPONDER_TRANSPORT_MESSAGE, &mut bob_buf)?;
  let alice_plaintext_len =
    alice.receive(&bob_buf[.. bob_to_alice_ciphertext_len], &mut alice_buf)?;
  let alice_plaintext = copy_plaintext(&alice_buf[.. alice_plaintext_len])?;

  Ok(DemoOutcome {
    first_handshake_message_len,
    second_handshake_message_len,
    alice_to_bob_ciphertext_len,
    bob_to_alice_ciphertext_len,
    bob_plaintext,
    bob_plaintext_len,
    alice_plaintext,
    alice_plaintext_len,
  })
}

/// Run the exchange with Clatter's `getrandom` backed default RNG.
///
/// This is intended for the host-side example binary. For a strict `no_std`
/// target, call [`run_hybrid_exchange_with_rng`] with a platform RNG instead.
#[cfg(feature = "system-rng")]
pub fn run_hybrid_exchange() -> Result<DemoOutcome, DemoError> {
  run_hybrid_exchange_with_rng::<clatter::crypto::rng::DefaultRng>()
}

fn copy_plaintext(src: &[u8]) -> Result<[u8; MAX_TRANSPORT_PAYLOAD_LEN], DemoError> {
  if src.len() > MAX_TRANSPORT_PAYLOAD_LEN {
    return Err(DemoError::PlaintextTooLarge);
  }

  let mut out = [0_u8; MAX_TRANSPORT_PAYLOAD_LEN];
  out[.. src.len()].copy_from_slice(src);
  Ok(out)
}

#[cfg(all(test, feature = "system-rng"))]
mod tests {
  use super::*;

  #[test]
  fn default_rng_exchange_round_trips_transport_messages() {
    let outcome = run_hybrid_exchange().expect("hybrid exchange should complete");

    assert_eq!(outcome.bob_plaintext(), INITIATOR_TRANSPORT_MESSAGE);
    assert_eq!(outcome.alice_plaintext(), RESPONDER_TRANSPORT_MESSAGE);
    assert!(outcome.first_handshake_message_len > 0);
    assert!(outcome.second_handshake_message_len > 0);
    assert!(outcome.alice_to_bob_ciphertext_len > INITIATOR_TRANSPORT_MESSAGE.len());
    assert!(outcome.bob_to_alice_ciphertext_len > RESPONDER_TRANSPORT_MESSAGE.len());
  }
}
