//! Player client node.
//!
//! Run (in a separate terminal, *after* map_node is running):
//!   cargo run --bin player_node
//!
//! The node boots a libp2p swarm (mDNS), spawns a PlayerActor, waits for the
//! map to appear in the swarm registry, then joins the map and exercises a hit.
//!
//! TCP disconnect simulation
//! ─────────────────────────
//! If you kill the player_node process and restart it within 180 seconds, the
//! map node will reconnect the player with its server-side snapshot intact (any
//! damage taken while offline is preserved — the server is authoritative).

#[path = "../map.rs"]
mod map;
#[path = "../player.rs"]
mod player;

use std::time::Duration;

use kameo::{actor::RemoteActorRef, prelude::*, remote};
use map::{Damage, GetAllPlayers, HitPlayer, MapActor};
use player::{EnterMap, GetPlayerSnapshot, PlayerActor, PlayerSnapshot, PlayerStats};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let peer_id = remote::bootstrap()?;
  println!("[player_node] peer id: {peer_id}");

  // ── Spawn and register the player ────────────────────────────────────────
  let player_id: u64 = 1001;
  let player = PlayerActor::spawn(PlayerActor {
    snapshot: PlayerSnapshot {
      id: player_id,
      name: "Knight".to_string(),
      stats: PlayerStats { red: 100, blue: 80 },
    },
  });
  let player_name = format!("player:{player_id}");
  player.register(player_name.as_str()).await?;
  println!("[player_node] PlayerActor registered as \"{player_name}\"");

  // ── Wait for the map node to appear via mDNS ──────────────────────────────
  println!("[player_node] looking for map …");
  let map_remote: RemoteActorRef<MapActor> = loop {
    match RemoteActorRef::<MapActor>::lookup("map").await? {
      Some(r) => break r,
      None => tokio::time::sleep(Duration::from_millis(500)).await,
    }
  };
  println!("[player_node] found map — joining …");

  // ── Join the map ─────────────────────────────────────────────────────────
  let join_info = player
    .ask(EnterMap {
      map: map_remote.clone(),
    })
    .await?;
  println!(
    "[player_node] joined; own snapshot: {:?} | {} other(s): {:?}",
    join_info.own_snapshot,
    join_info.other_players.len(),
    join_info
      .other_players
      .iter()
      .map(|p| p.name.as_str())
      .collect::<Vec<_>>(),
  );

  // ── Apply a hit (could be from any node) ─────────────────────────────────
  // In a real game the game server / map node would trigger HitPlayer; here
  // we send it directly for demonstration purposes.
  tokio::time::sleep(Duration::from_millis(200)).await;
  let hit = map_remote
    .ask(&HitPlayer {
      player_id,
      damage: Damage { red: 15, blue: 5 },
    })
    .await?
    .expect("player exists in map");
  println!("[player_node] hit report: {hit:?}");

  // ── Give the map time to push SyncFromMap back ───────────────────────────
  tokio::time::sleep(Duration::from_millis(100)).await;

  // ── Full room snapshot ───────────────────────────────────────────────────
  let all = map_remote.ask(&GetAllPlayers).await?;
  println!("[player_node] room snapshot: {all:?}");

  let snap = player.ask(GetPlayerSnapshot).await?;
  println!("[player_node] local actor snapshot: {snap:?}");

  // ── Assertions ───────────────────────────────────────────────────────────
  assert_eq!(snap.stats.red, hit.stats_after_hit.red);
  assert_eq!(snap.stats.blue, hit.stats_after_hit.blue);

  // ── Graceful shutdown ────────────────────────────────────────────────────
  player.stop_gracefully().await?;
  player.wait_for_shutdown().await;
  Ok(())
}
