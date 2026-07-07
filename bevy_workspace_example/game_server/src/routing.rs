use tokio::sync::{mpsc, oneshot};

use crate::protocol::{ClientEnvelope, ServerEnvelope};

pub(crate) const GATEWAY_EVENT_BUFFER: usize = 65_536;
pub(crate) const SHARD_COMMAND_BUFFER: usize = 16_384;
pub(crate) const CLIENT_OUTBOUND_BUFFER: usize = 8;

#[derive(Clone)]
pub(crate) struct ShardHandle {
  pub(crate) id: usize,
  pub(crate) sender: mpsc::Sender<ShardCommand>,
}

pub(crate) enum ShardCommand {
  Connected {
    client_id: u64,
  },
  Message {
    client_id: u64,
    message: ClientEnvelope,
  },
  Disconnected {
    client_id: u64,
  },
}

pub(crate) enum GatewayEvent {
  ClientReady {
    client_id: u64,
    shard_id: usize,
    sender: mpsc::Sender<ServerEnvelope>,
    ack: oneshot::Sender<()>,
  },
  Send {
    client_id: u64,
    message: ServerEnvelope,
  },
  ClientDisconnected {
    client_id: u64,
    shard_id: usize,
  },
}
