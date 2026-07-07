mod agones;
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

  if let Err(err) = net::run_gateway(bind_addr().as_str(), shards, gateway_receiver) {
    eprintln!("game_server gateway error: {err:#}");
  }
}

fn bind_addr() -> String {
  std::env::var("GAME_SERVER_BIND")
    .or_else(|_| std::env::var("GAME_SERVER_ADDR"))
    .unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string())
}

fn shard_count() -> usize {
  std::env::var("GAME_SERVER_SHARDS")
    .ok()
    .and_then(|value| value.parse::<usize>().ok())
    .filter(|count| *count > 0)
    .unwrap_or(shard::DEFAULT_SHARD_COUNT)
}
