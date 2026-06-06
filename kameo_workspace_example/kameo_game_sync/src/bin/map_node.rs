//! Map server node.
//!
//! Run:
//!   cargo run --bin map_node
//!
//! The node boots a libp2p swarm (mDNS peer discovery), spawns the MapActor,
//! and registers it as "map" in the swarm registry.  Player nodes on the same
//! LAN discover it via mDNS and can send `EnterPlayer`, `HitPlayer`, etc.
//!
//! Press Ctrl-C to shut down.

#[path = "../map.rs"]
mod map;
#[path = "../player.rs"]
mod player;

use kameo::{prelude::*, remote};
use map::MapActor;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Listen on all interfaces, OS-assigned TCP port (mDNS announces it).
  let peer_id = remote::bootstrap()?;
  println!("[map_node] peer id: {peer_id}");

  let map = MapActor::spawn(MapActor::default());
  map.register("map").await?;
  println!("[map_node] MapActor registered as \"map\" — waiting for players …");

  // Keep running until Ctrl-C.
  tokio::signal::ctrl_c().await?;
  println!("[map_node] shutting down …");

  map.stop_gracefully().await?;
  map.wait_for_shutdown().await;
  Ok(())
}
