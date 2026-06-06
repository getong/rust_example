//! Single-process demo — both map and player actors run locally, but the full
//! remote infrastructure (libp2p swarm, registry) is bootstrapped so that the
//! same message-handler code works without modification when actors are split
//! across real TCP nodes (see src/bin/map_node.rs and src/bin/player_node.rs).

mod map;
mod player;

use kameo::{prelude::*, remote};
use map::{Damage, GetAllPlayers, HitPlayer, MapActor};
use player::{EnterMap, GetPlayerView, InitialMapStats, PlayerActor, PlayerProfile};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // ── Bootstrap the libp2p swarm (mDNS, random OS port) ───────────────────
  // Required even in single-node mode so that `into_remote_ref()` and
  // registry-based lookups work.
  remote::bootstrap()?;

  // ── Spawn actors ─────────────────────────────────────────────────────────
  let map = MapActor::spawn(MapActor::default());

  let player1 = PlayerActor::spawn(PlayerActor {
    profile: PlayerProfile {
      id: 1001,
      name: "Knight".to_string(),
    },
    initial_map_stats: InitialMapStats { red: 100, blue: 80 },
    map_mirror: None,
  });

  let player2 = PlayerActor::spawn(PlayerActor {
    profile: PlayerProfile {
      id: 1002,
      name: "Mage".to_string(),
    },
    initial_map_stats: InitialMapStats { red: 60, blue: 150 },
    map_mirror: None,
  });

  // ── Register actors in the swarm registry ────────────────────────────────
  // The map uses "player:{id}" names to push stats back to players via
  // RemoteActorRef::lookup — even when they live on the same node.
  map.register("map").await?;
  player1.register("player:1001").await?;
  player2.register("player:1002").await?;

  // ── Convert local ActorRef → RemoteActorRef ───────────────────────────────
  // `into_remote_ref()` also re-registers the actor in the registry.
  let map_remote = map.into_remote_ref().await;

  // ── Both players join the map concurrently ────────────────────────────────
  let (res1, res2) = tokio::join!(
    player1.ask(EnterMap {
      map: map_remote.clone()
    }),
    player2.ask(EnterMap {
      map: map_remote.clone()
    }),
  );
  let join1 = res1?;
  let join2 = res2?;

  println!(
    "[main] player1 joined; sees {} other(s): {:?}",
    join1.other_players.len(),
    join1
      .other_players
      .iter()
      .map(|p| p.profile.name.as_str())
      .collect::<Vec<_>>(),
  );
  println!(
    "[main] player2 joined; sees {} other(s): {:?}",
    join2.other_players.len(),
    join2
      .other_players
      .iter()
      .map(|p| p.profile.name.as_str())
      .collect::<Vec<_>>(),
  );

  // ── Hit player1 ──────────────────────────────────────────────────────────
  let hit = map
    .ask(HitPlayer {
      player_id: 1001,
      damage: Damage { red: 23, blue: 9 },
    })
    .await?
    .expect("player1 exists in map");
  println!("[main] hit report: {hit:?}");

  // Give the background sync task a moment to push stats back to player1.
  tokio::time::sleep(std::time::Duration::from_millis(50)).await;

  // ── Full room sync (simulates a late-joining client) ─────────────────────
  let all_players = map.ask(GetAllPlayers).await?;
  println!("[main] all players in map (client sync): {all_players:?}");

  let p1_view = player1.ask(GetPlayerView).await?;
  let p2_view = player2.ask(GetPlayerView).await?;
  println!("[main] player1 actor view: {p1_view:?}");
  println!("[main] player2 actor view: {p2_view:?}");

  // ── Assertions ───────────────────────────────────────────────────────────
  assert_eq!(hit.player_id, 1001);
  let p1_mirror = p1_view.map_mirror.expect("player1 has map mirror");
  let p2_mirror = p2_view.map_mirror.expect("player2 has map mirror");
  assert_eq!(p1_mirror.own_state.stats.red, hit.state_after_hit.stats.red);
  assert_eq!(
    p1_mirror.own_state.stats.blue,
    hit.state_after_hit.stats.blue
  );

  let map_p1 = all_players
    .iter()
    .find(|p| p.profile.id == 1001)
    .expect("player1 in map");
  assert_eq!(map_p1.stats.red, p1_mirror.own_state.stats.red);
  assert_eq!(map_p1.stats.blue, p1_mirror.own_state.stats.blue);

  let map_p2 = all_players
    .iter()
    .find(|p| p.profile.id == 1002)
    .expect("player2 in map");
  assert_eq!(map_p2.stats.red, p2_mirror.own_state.stats.red);
  assert_eq!(map_p2.stats.blue, p2_mirror.own_state.stats.blue);

  // ── Shutdown ─────────────────────────────────────────────────────────────
  map.stop_gracefully().await?;
  player1.stop_gracefully().await?;
  player2.stop_gracefully().await?;
  map.wait_for_shutdown().await;
  player1.wait_for_shutdown().await;
  player2.wait_for_shutdown().await;

  Ok(())
}
