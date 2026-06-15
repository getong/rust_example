use snow::{Builder, Error, params::NoiseParams};

const NOISE_XXHFS_MLKEM768: &str = "Noise_XXhfs_25519+ML-KEM-768_ChaChaPoly_SHA256";
const PROLOGUE: &[u8] = b"snow post-quantum hybrid demo";
const MESSAGE: &[u8] = b"harvest now, decrypt never";

fn main() -> Result<(), Error> {
  let params: NoiseParams = NOISE_XXHFS_MLKEM768.parse()?;

  let initiator_static = Builder::new(params.clone()).generate_keypair()?;
  let responder_static = Builder::new(params.clone()).generate_keypair()?;

  let mut initiator = Builder::new(params.clone())
    .local_private_key(&initiator_static.private)?
    .prologue(PROLOGUE)?
    .build_initiator()?;
  let mut responder = Builder::new(params)
    .local_private_key(&responder_static.private)?
    .prologue(PROLOGUE)?
    .build_responder()?;

  let handshake_lens = run_xx_handshake(&mut initiator, &mut responder)?;

  let initiator_remote_static = initiator.get_remote_static().map(Vec::from);
  let responder_remote_static = responder.get_remote_static().map(Vec::from);

  let mut initiator = initiator.into_transport_mode()?;
  let mut responder = responder.into_transport_mode()?;

  assert_eq!(
    initiator_remote_static.as_deref(),
    Some(responder_static.public.as_slice())
  );
  assert_eq!(
    responder_remote_static.as_deref(),
    Some(initiator_static.public.as_slice())
  );

  let ciphertext_len = send_and_receive(&mut initiator, &mut responder, MESSAGE)?;
  let reply_len = send_and_receive(&mut responder, &mut initiator, b"ack")?;

  println!("protocol: {NOISE_XXHFS_MLKEM768}");
  println!("handshake messages: {:?}", handshake_lens);
  println!("initiator -> responder ciphertext bytes: {ciphertext_len}");
  println!("responder -> initiator ciphertext bytes: {reply_len}");
  println!(
    "decrypted application message: {}",
    String::from_utf8_lossy(MESSAGE)
  );

  Ok(())
}

fn run_xx_handshake(
  initiator: &mut snow::HandshakeState,
  responder: &mut snow::HandshakeState,
) -> Result<[usize; 3], Error> {
  let mut msg = [0_u8; 4096];
  let mut out = [0_u8; 4096];

  let first = initiator.write_message(&[], &mut msg)?;
  responder.read_message(&msg[.. first], &mut out)?;

  let second = responder.write_message(&[], &mut msg)?;
  initiator.read_message(&msg[.. second], &mut out)?;

  let third = initiator.write_message(&[], &mut msg)?;
  responder.read_message(&msg[.. third], &mut out)?;

  Ok([first, second, third])
}

fn send_and_receive(
  sender: &mut snow::TransportState,
  receiver: &mut snow::TransportState,
  plaintext: &[u8],
) -> Result<usize, Error> {
  let mut ciphertext = [0_u8; 4096];
  let mut decrypted = [0_u8; 4096];

  let ciphertext_len = sender.write_message(plaintext, &mut ciphertext)?;
  let plaintext_len = receiver.read_message(&ciphertext[.. ciphertext_len], &mut decrypted)?;

  assert_eq!(&decrypted[.. plaintext_len], plaintext);

  Ok(ciphertext_len)
}
// copy from https://github.com/libp2p/rust-libp2p/pull/6481