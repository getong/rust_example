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
#[path = "../tcp_monitor.rs"]
mod tcp_monitor;

use std::time::Duration;

use kameo::{actor::RemoteActorRef, prelude::*, remote};
use map::{Damage, GetAllPlayers, HitPlayer, MapActor};
use player::{EnterMap, GetPlayerView, InitialMapStats, PlayerActor, PlayerProfile};
use tcp_monitor::{BindTcpConnection, GetTcpConnections, TcpConnectionMonitor, TcpDisconnected};

struct PlayerConfig {
  id: u64,
  name: &'static str,
  red: i32,
  blue: i32,
}

struct PlayerRuntime {
  id: u64,
  actor: ActorRef<PlayerActor>,
  connection_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let peer_id = remote::bootstrap()?;
  println!("[player_node] peer id: {peer_id}");

  let tcp_monitor =
    TcpConnectionMonitor::spawn(TcpConnectionMonitor::new(Duration::from_secs(180)));

  let player_configs = [
    PlayerConfig {
      id: 1001,
      name: "Knight",
      red: 100,
      blue: 80,
    },
    PlayerConfig {
      id: 1002,
      name: "Mage",
      red: 60,
      blue: 150,
    },
    PlayerConfig {
      id: 1003,
      name: "Archer",
      red: 85,
      blue: 95,
    },
  ];

  // ── Spawn, register, and bind players to TCP sessions ────────────────────
  let mut players = Vec::new();
  for config in player_configs {
    let player = PlayerActor::spawn(PlayerActor {
      profile: PlayerProfile {
        id: config.id,
        name: config.name.to_string(),
      },
      initial_map_stats: InitialMapStats {
        red: config.red,
        blue: config.blue,
      },
      map_mirror: None,
    });
    let player_name = format!("player:{}", config.id);
    player.register(player_name.as_str()).await?;
    println!("[player_node] PlayerActor registered as \"{player_name}\"");

    let connection_id = format!("tcp:{}:session-1", config.id);
    let bind = tcp_monitor
      .ask(BindTcpConnection {
        player_id: config.id,
        connection_id: connection_id.clone(),
        player: player.clone(),
      })
      .await?;
    bind.log();

    players.push(PlayerRuntime {
      id: config.id,
      actor: player,
      connection_id,
    });
  }

  // ── Wait for the map node to appear via mDNS ──────────────────────────────
  println!("[player_node] looking for map …");
  let map_remote: RemoteActorRef<MapActor> = loop {
    match RemoteActorRef::<MapActor>::lookup("map").await? {
      Some(r) => break r,
      None => tokio::time::sleep(Duration::from_millis(500)).await,
    }
  };
  println!("[player_node] found map — joining …");

  // ── Join all players to the map ──────────────────────────────────────────
  for runtime in &players {
    let join_info = runtime
      .actor
      .ask(EnterMap {
        map: map_remote.clone(),
      })
      .await?;
    println!(
      "[player_node] player:{} joined; authoritative map state: {:?} | {} other(s): {:?}",
      runtime.id,
      join_info.own_state,
      join_info.other_players.len(),
      join_info
        .other_players
        .iter()
        .map(|p| p.profile.name.as_str())
        .collect::<Vec<_>>(),
    );
  }

  // ── Apply a hit (could be from any node) ─────────────────────────────────
  // In a real game the game server / map node would trigger HitPlayer; here
  // we send it directly for demonstration purposes.
  tokio::time::sleep(Duration::from_millis(200)).await;
  let hit = map_remote
    .ask(&HitPlayer {
      player_id: players[0].id,
      damage: Damage { red: 15, blue: 5 },
    })
    .await?
    .expect("player exists in map");
  println!("[player_node] hit report: {hit:?}");

  // ── Simulate one TCP session dropping and reconnecting within 180 s ──────
  let reconnect_player = &mut players[1];
  let disconnected = tcp_monitor
    .ask(TcpDisconnected {
      player_id: reconnect_player.id,
      connection_id: reconnect_player.connection_id.clone(),
    })
    .await?;
  println!(
    "[player_node] simulated tcp disconnect for player:{}: {disconnected}",
    reconnect_player.id
  );

  tokio::time::sleep(Duration::from_millis(200)).await;
  reconnect_player.connection_id = format!("tcp:{}:session-2", reconnect_player.id);
  let rebind = tcp_monitor
    .ask(BindTcpConnection {
      player_id: reconnect_player.id,
      connection_id: reconnect_player.connection_id.clone(),
      player: reconnect_player.actor.clone(),
    })
    .await?;
  rebind.log();

  // ── Simulate one TCP session dropping and not reconnecting ───────────────
  let dropped_player = &players[2];
  let dropped = tcp_monitor
    .ask(TcpDisconnected {
      player_id: dropped_player.id,
      connection_id: dropped_player.connection_id.clone(),
    })
    .await?;
  println!(
    "[player_node] simulated tcp disconnect without reconnect for player:{}: {dropped}",
    dropped_player.id
  );

  // ── Give the map time to push map events back ────────────────────────────
  tokio::time::sleep(Duration::from_millis(100)).await;

  // ── Full room snapshot ───────────────────────────────────────────────────
  let all = map_remote.ask(&GetAllPlayers).await?;
  println!("[player_node] room snapshot: {all:?}");

  let tcp_snapshot = tcp_monitor.ask(GetTcpConnections).await?;
  for connection in &tcp_snapshot {
    connection.log();
  }

  for runtime in &players {
    let view = runtime.actor.ask(GetPlayerView).await?;
    println!(
      "[player_node] local actor view for player:{}: {view:?}",
      runtime.id
    );

    if runtime.id == hit.player_id {
      let mirror = view.map_mirror.expect("hit player has map mirror");
      assert_eq!(mirror.own_state.stats.red, hit.state_after_hit.stats.red);
      assert_eq!(mirror.own_state.stats.blue, hit.state_after_hit.stats.blue);
    }
  }

  println!(
    "[player_node] staying alive; player:{} will be stopped by tcp monitor after 180 s if it does \
     not reconnect. Press Ctrl-C to shut down.",
    dropped_player.id
  );
  tokio::signal::ctrl_c().await?;
  println!("[player_node] shutting down …");

  // ── Graceful shutdown ────────────────────────────────────────────────────
  tcp_monitor.stop_gracefully().await?;
  tcp_monitor.wait_for_shutdown().await;

  for runtime in players {
    runtime.actor.stop_gracefully().await?;
    runtime.actor.wait_for_shutdown().await;
  }
  Ok(())
}
