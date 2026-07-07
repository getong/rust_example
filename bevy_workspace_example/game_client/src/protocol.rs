use anyhow::{Context, Result, bail};
use bevy::prelude::{App, Plugin};
use lightyear::prelude::{
  AppChannelExt, AppMessageExt, ChannelMode, ChannelSettings, NetworkDirection, ReliableSettings,
};
use prost::Message;
use serde::{Deserialize, Serialize};

mod generated {
  include!(concat!(env!("OUT_DIR"), "/game.rs"));
}

pub use generated::*;

pub const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:6000";
pub const NETCODE_PROTOCOL_ID: u64 = 0x1f2e_3d4c_5b6a_7988;
pub const NETCODE_PRIVATE_KEY: [u8; 32] = [
  7, 44, 91, 128, 3, 219, 17, 64, 55, 222, 105, 18, 87, 144, 23, 201, 42, 11, 76, 190, 6, 95, 231,
  12, 166, 37, 73, 154, 208, 1, 62, 119,
];

const MAX_FRAME_SIZE: usize = 64 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientPacket {
  payload: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerPacket {
  payload: Vec<u8>,
}

pub struct GameChannel;

pub struct GameProtocolPlugin;

impl Plugin for GameProtocolPlugin {
  fn build(&self, app: &mut App) {
    app
      .add_channel::<GameChannel>(ChannelSettings {
        mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
        ..Default::default()
      })
      .add_direction(NetworkDirection::Bidirectional);
    app
      .register_message::<ClientPacket>()
      .add_direction(NetworkDirection::ClientToServer);
    app
      .register_message::<ServerPacket>()
      .add_direction(NetworkDirection::ServerToClient);
  }
}

pub fn encode_client_envelope(message: &ClientEnvelope) -> Result<ClientPacket> {
  Ok(ClientPacket {
    payload: encode_envelope(message)?,
  })
}

pub fn decode_client_packet(packet: ClientPacket) -> Result<ClientEnvelope> {
  decode_envelope(&packet.payload)
}

pub fn encode_server_envelope(message: &ServerEnvelope) -> Result<ServerPacket> {
  Ok(ServerPacket {
    payload: encode_envelope(message)?,
  })
}

pub fn decode_server_packet(packet: ServerPacket) -> Result<ServerEnvelope> {
  decode_envelope(&packet.payload)
}

fn encode_envelope<M>(message: &M) -> Result<Vec<u8>>
where
  M: Message,
{
  let payload = message.encode_to_vec();
  validate_frame_len(payload.len())?;
  Ok(payload)
}

fn decode_envelope<M>(payload: &[u8]) -> Result<M>
where
  M: Message + Default,
{
  validate_frame_len(payload.len())?;
  M::decode(payload).context("failed to decode protobuf frame")
}

fn validate_frame_len(payload_len: usize) -> Result<()> {
  if payload_len > MAX_FRAME_SIZE {
    bail!(
      "protobuf frame length {} exceeds limit {MAX_FRAME_SIZE}",
      payload_len
    );
  }
  Ok(())
}
