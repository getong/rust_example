mod behavior;
mod game;
mod net;
pub mod protocol;
mod routing;
mod shard;
mod terrain;

use tokio::sync::mpsc;

use crate::{protocol::DEFAULT_SERVER_ADDR, routing::GATEWAY_EVENT_BUFFER};

pub fn run() {
  let shard_count = shard_count();
  let (gateway_sender, gateway_receiver) = mpsc::channel(GATEWAY_EVENT_BUFFER);
  let shards = shard::spawn_shards(shard_count, gateway_sender.clone());

  if let Err(err) = net::run_gateway(
    DEFAULT_SERVER_ADDR,
    shards,
    gateway_sender,
    gateway_receiver,
  ) {
    eprintln!("game_server gateway error: {err:#}");
  }
}

fn shard_count() -> usize {
  std::env::var("GAME_SERVER_SHARDS")
    .ok()
    .and_then(|value| value.parse::<usize>().ok())
    .filter(|count| *count > 0)
    .unwrap_or(shard::DEFAULT_SHARD_COUNT)
}
