use anyhow::{Context, Result, bail};
use prost::Message;
use quinn::{ReadExactError, RecvStream, SendStream};

mod generated {
  include!(concat!(env!("OUT_DIR"), "/game.rs"));
}

pub use generated::*;

pub const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:6000";
pub const ALPN_PROTOCOL: &[u8] = b"bevy-game/0";
const MAX_FRAME_SIZE: usize = 64 * 1024;

pub async fn read_client_envelope(recv: &mut RecvStream) -> Result<Option<ClientEnvelope>> {
  read_envelope(recv).await
}

pub async fn read_server_envelope(recv: &mut RecvStream) -> Result<Option<ServerEnvelope>> {
  read_envelope(recv).await
}

pub async fn write_client_envelope(send: &mut SendStream, message: &ClientEnvelope) -> Result<()> {
  write_envelope(send, message).await
}

pub async fn write_server_envelope(send: &mut SendStream, message: &ServerEnvelope) -> Result<()> {
  write_envelope(send, message).await
}

async fn read_envelope<M>(recv: &mut RecvStream) -> Result<Option<M>>
where
  M: Message + Default,
{
  let Some(frame) = read_frame(recv).await? else {
    return Ok(None);
  };

  M::decode(frame.as_slice())
    .map(Some)
    .context("failed to decode protobuf frame")
}

async fn write_envelope<M>(send: &mut SendStream, message: &M) -> Result<()>
where
  M: Message,
{
  let payload = message.encode_to_vec();
  write_frame(send, &payload).await
}

async fn read_frame(recv: &mut RecvStream) -> Result<Option<Vec<u8>>> {
  let mut len_buf = [0_u8; 4];
  match recv.read_exact(&mut len_buf).await {
    Ok(()) => {}
    Err(ReadExactError::FinishedEarly(0)) => return Ok(None),
    Err(err) => return Err(err).context("failed to read protobuf frame length"),
  }

  let len = u32::from_be_bytes(len_buf) as usize;
  if len > MAX_FRAME_SIZE {
    bail!("protobuf frame length {len} exceeds limit {MAX_FRAME_SIZE}");
  }

  let mut payload = vec![0_u8; len];
  recv
    .read_exact(&mut payload)
    .await
    .context("failed to read protobuf frame payload")?;
  Ok(Some(payload))
}

async fn write_frame(send: &mut SendStream, payload: &[u8]) -> Result<()> {
  if payload.len() > MAX_FRAME_SIZE {
    bail!(
      "protobuf frame length {} exceeds limit {MAX_FRAME_SIZE}",
      payload.len()
    );
  }

  let len = (payload.len() as u32).to_be_bytes();
  send
    .write_all(&len)
    .await
    .context("failed to write protobuf frame length")?;
  send
    .write_all(payload)
    .await
    .context("failed to write protobuf frame payload")?;
  Ok(())
}
