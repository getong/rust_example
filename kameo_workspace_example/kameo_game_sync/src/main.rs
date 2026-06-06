mod map;
mod player;
mod tcp_monitor;

use std::{collections::HashSet, time::Duration};

use clap::Parser;
use kameo::{actor::RemoteActorRef, prelude::*, remote};
use map::{Damage, GetAllPlayers, HitPlayer, MapActor};
use player::{EnterMap, GetPlayerView, InitialMapStats, PlayerActor, PlayerId, PlayerProfile};
use rand::{RngExt, SeedableRng, rngs::StdRng};
use tcp_monitor::{BindTcpConnection, GetTcpConnections, TcpConnectionMonitor, TcpDisconnected};
use tokio::task::JoinHandle;

#[derive(Parser, Debug)]
#[command(
  name = "kameo_game_sync",
  about = "Start one game node containing both map and assigned player actors"
)]
struct Cli {
  /// Current logical node id, e.g. node-a.
  #[arg(long)]
  node_id: String,

  /// Comma-separated logical node ids participating in player assignment.
  #[arg(long, value_delimiter = ',', default_value = "node-a")]
  nodes: Vec<String>,

  /// Deterministic RNG seed used to assign players to nodes.
  #[arg(long, default_value_t = 7)]
  seed: u64,

  /// Run the startup demo once and exit instead of keeping the node alive.
  #[arg(long, default_value_t = false)]
  run_once: bool,
}

#[derive(Clone, Copy)]
struct PlayerConfig {
  id: PlayerId,
  name: &'static str,
  red: i32,
  blue: i32,
}

struct PlayerRuntime {
  id: PlayerId,
  actor: ActorRef<PlayerActor>,
  connection_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cli = Cli::parse();
  validate_nodes(&cli)?;

  let peer_id = remote::bootstrap()?;
  println!(
    "[node:{}] peer id: {peer_id}; assignment seed: {}",
    cli.node_id, cli.seed
  );

  let map = MapActor::spawn(MapActor::default());
  let map_name = format!("map:{}", cli.node_id);
  map.register(map_name.as_str()).await?;
  let map_remote = map.into_remote_ref().await;
  println!(
    "[node:{}] MapActor registered as \"{map_name}\"",
    cli.node_id
  );
  let peer_discovery = spawn_peer_map_discovery(cli.node_id.clone(), cli.nodes.clone());

  let tcp_monitor =
    TcpConnectionMonitor::spawn(TcpConnectionMonitor::new(Duration::from_secs(180)));

  let assignments = assign_players(&player_configs(), &cli.nodes, cli.seed);
  print_assignments(&cli.node_id, &assignments);

  let local_players: Vec<PlayerConfig> = assignments
    .iter()
    .filter_map(|(player, node_id)| (node_id == &cli.node_id).then_some(*player))
    .collect();

  let mut players = spawn_local_players(&cli.node_id, local_players, &tcp_monitor).await?;

  for runtime in &players {
    let join_info = runtime
      .actor
      .ask(EnterMap {
        map: map_remote.clone(),
      })
      .await?;
    println!(
      "[node:{}] player:{} joined local map; own state: {:?} | {} other(s): {:?}",
      cli.node_id,
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

  if let Some(first_player) = players.first() {
    tokio::time::sleep(Duration::from_millis(200)).await;
    let hit = map
      .ask(HitPlayer {
        player_id: first_player.id,
        damage: Damage { red: 15, blue: 5 },
      })
      .await?
      .expect("local player exists in map");
    println!("[node:{}] hit report: {hit:?}", cli.node_id);

    tokio::time::sleep(Duration::from_millis(100)).await;
    let view = first_player.actor.ask(GetPlayerView).await?;
    let mirror = view.map_mirror.expect("hit player has map mirror");
    assert_eq!(mirror.own_state.stats.red, hit.state_after_hit.stats.red);
    assert_eq!(mirror.own_state.stats.blue, hit.state_after_hit.stats.blue);
  } else {
    println!(
      "[node:{}] no players were assigned to this node; only map is running",
      cli.node_id
    );
  }

  if players.len() >= 2 {
    let reconnect_player = &mut players[1];
    let disconnected = tcp_monitor
      .ask(TcpDisconnected {
        player_id: reconnect_player.id,
        connection_id: reconnect_player.connection_id.clone(),
      })
      .await?;
    println!(
      "[node:{}] simulated tcp disconnect for player:{}: {disconnected}",
      cli.node_id, reconnect_player.id
    );

    tokio::time::sleep(Duration::from_millis(200)).await;
    reconnect_player.connection_id =
      format!("tcp:{}:{}:session-2", cli.node_id, reconnect_player.id);
    let rebind = tcp_monitor
      .ask(BindTcpConnection {
        player_id: reconnect_player.id,
        connection_id: reconnect_player.connection_id.clone(),
        player: reconnect_player.actor.clone(),
      })
      .await?;
    rebind.log();
  }

  if players.len() >= 3 {
    let dropped_player = &players[2];
    let dropped = tcp_monitor
      .ask(TcpDisconnected {
        player_id: dropped_player.id,
        connection_id: dropped_player.connection_id.clone(),
      })
      .await?;
    println!(
      "[node:{}] simulated tcp disconnect without reconnect for player:{}: {dropped}",
      cli.node_id, dropped_player.id
    );
  }

  tokio::time::sleep(Duration::from_millis(100)).await;

  let all_players = map.ask(GetAllPlayers).await?;
  println!(
    "[node:{}] local map authoritative snapshot: {all_players:?}",
    cli.node_id
  );

  let tcp_snapshot = tcp_monitor.ask(GetTcpConnections).await?;
  for connection in &tcp_snapshot {
    connection.log();
  }

  for runtime in &players {
    let view = runtime.actor.ask(GetPlayerView).await?;
    println!(
      "[node:{}] local player view for player:{}: {view:?}",
      cli.node_id, runtime.id
    );
  }

  if cli.run_once {
    abort_peer_discovery(peer_discovery);
    shutdown(map, tcp_monitor, players).await?;
    return Ok(());
  }

  println!(
    "[node:{}] running in background actor mode. Press Ctrl-C to shut down.",
    cli.node_id
  );
  tokio::signal::ctrl_c().await?;
  println!("[node:{}] shutting down ...", cli.node_id);

  abort_peer_discovery(peer_discovery);
  shutdown(map, tcp_monitor, players).await?;
  Ok(())
}

fn spawn_peer_map_discovery(node_id: String, nodes: Vec<String>) -> Option<JoinHandle<()>> {
  let peer_nodes: Vec<String> = nodes.into_iter().filter(|node| node != &node_id).collect();
  if peer_nodes.is_empty() {
    return None;
  }

  println!(
    "[node:{node_id}] peer map discovery watching: {}",
    peer_nodes.join(", ")
  );

  Some(tokio::spawn(async move {
    let mut connected = HashSet::new();
    let mut logged_errors = HashSet::new();

    loop {
      for peer_node in &peer_nodes {
        let map_name = format!("map:{peer_node}");
        match RemoteActorRef::<MapActor>::lookup(map_name.as_str()).await {
          Ok(Some(peer_map)) => match peer_map.ask(&GetAllPlayers).await {
            Ok(players) => {
              logged_errors.remove(peer_node);
              if connected.insert(peer_node.clone()) {
                println!(
                  "[node:{node_id}] connected to peer map \"{map_name}\" ({} player(s))",
                  players.len()
                );
              }
            }
            Err(err) => {
              if connected.remove(peer_node) {
                println!("[node:{node_id}] lost peer map \"{map_name}\"");
              }
              if logged_errors.insert(peer_node.clone()) {
                eprintln!("[node:{node_id}] peer map ask failed for \"{map_name}\": {err}");
              }
            }
          },
          Ok(None) => {
            logged_errors.remove(peer_node);
            if connected.remove(peer_node) {
              println!("[node:{node_id}] lost peer map \"{map_name}\"");
            }
          }
          Err(err) => {
            if connected.remove(peer_node) {
              println!("[node:{node_id}] lost peer map \"{map_name}\"");
            }
            if logged_errors.insert(peer_node.clone()) {
              eprintln!("[node:{node_id}] peer map lookup failed for \"{map_name}\": {err}");
            }
          }
        }
      }

      tokio::time::sleep(Duration::from_secs(2)).await;
    }
  }))
}

fn abort_peer_discovery(peer_discovery: Option<JoinHandle<()>>) {
  if let Some(handle) = peer_discovery {
    handle.abort();
  }
}

fn validate_nodes(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
  if cli.nodes.is_empty() {
    return Err("at least one node id must be supplied with --nodes".into());
  }
  if !cli.nodes.iter().any(|node| node == &cli.node_id) {
    return Err(
      format!(
        "--node-id {} must be present in --nodes {:?}",
        cli.node_id, cli.nodes
      )
      .into(),
    );
  }
  Ok(())
}

fn player_configs() -> Vec<PlayerConfig> {
  vec![
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
    PlayerConfig {
      id: 1004,
      name: "Cleric",
      red: 75,
      blue: 130,
    },
    PlayerConfig {
      id: 1005,
      name: "Rogue",
      red: 90,
      blue: 70,
    },
  ]
}

fn assign_players(
  players: &[PlayerConfig],
  nodes: &[String],
  seed: u64,
) -> Vec<(PlayerConfig, String)> {
  let mut rng = StdRng::seed_from_u64(seed);
  players
    .iter()
    .copied()
    .map(|player| {
      let node_idx = rng.random_range(0 .. nodes.len());
      (player, nodes[node_idx].clone())
    })
    .collect()
}

fn print_assignments(node_id: &str, assignments: &[(PlayerConfig, String)]) {
  println!("[node:{node_id}] player assignment:");
  for (player, assigned_node) in assignments {
    println!(
      "[node:{node_id}]   player:{} ({}) -> {}",
      player.id, player.name, assigned_node
    );
  }
}

async fn spawn_local_players(
  node_id: &str,
  players: Vec<PlayerConfig>,
  tcp_monitor: &ActorRef<TcpConnectionMonitor>,
) -> Result<Vec<PlayerRuntime>, Box<dyn std::error::Error>> {
  let mut runtimes = Vec::new();
  for config in players {
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
    println!("[node:{node_id}] PlayerActor registered as \"{player_name}\"");

    let connection_id = format!("tcp:{node_id}:{}:session-1", config.id);
    let bind = tcp_monitor
      .ask(BindTcpConnection {
        player_id: config.id,
        connection_id: connection_id.clone(),
        player: player.clone(),
      })
      .await?;
    bind.log();

    runtimes.push(PlayerRuntime {
      id: config.id,
      actor: player,
      connection_id,
    });
  }
  Ok(runtimes)
}

async fn shutdown(
  map: ActorRef<MapActor>,
  tcp_monitor: ActorRef<TcpConnectionMonitor>,
  players: Vec<PlayerRuntime>,
) -> Result<(), Box<dyn std::error::Error>> {
  tcp_monitor.stop_gracefully().await?;
  tcp_monitor.wait_for_shutdown().await;

  for runtime in players {
    runtime.actor.stop_gracefully().await?;
    runtime.actor.wait_for_shutdown().await;
  }

  map.stop_gracefully().await?;
  map.wait_for_shutdown().await;
  Ok(())
}
