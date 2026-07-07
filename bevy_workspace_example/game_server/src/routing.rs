use tokio::sync::mpsc;

use crate::protocol::{ClientEnvelope, ServerEnvelope};

pub(crate) const GATEWAY_EVENT_BUFFER: usize = 65_536;
pub(crate) const SHARD_COMMAND_BUFFER: usize = 16_384;

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
  Send {
    client_id: u64,
    message: ServerEnvelope,
  },
}
