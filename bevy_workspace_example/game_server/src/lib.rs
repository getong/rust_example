mod agones;
mod behavior;
mod game;
mod net;
mod player_registry;
pub mod protocol;
mod replication;
mod terrain;

use crate::protocol::DEFAULT_SERVER_ADDR;

pub fn run() {
  if let Err(err) = net::run_server(bind_addr().as_str()) {
    eprintln!("game_server error: {err:#}");
  }
}

fn bind_addr() -> String {
  std::env::var("GAME_SERVER_BIND")
    .or_else(|_| std::env::var("GAME_SERVER_ADDR"))
    .unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string())
}
