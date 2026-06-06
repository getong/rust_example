mod map;
mod player;
mod tcp_monitor;

use std::{collections::HashSet, time::Duration};

use clap::Parser;
use kameo::{actor::RemoteActorRef, prelude::*, remote};
use map::{GetAllPlayers, GivePlayerBuff, HitPlayer, MapActor, MapConfig};
use player::{
  Damage, EnterMap, GetPlayerView, InitialCombatStats, InitialMapStats, PlayerActor, PlayerBuff,
  PlayerId, PlayerProfile,
};
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
  base_attack: Damage,
  base_damage: Damage,
}

struct PlayerRuntime {
  id: PlayerId,
  map_id: String,
  actor: ActorRef<PlayerActor>,
  connection_id: String,
}

struct MapRuntime {
  id: String,
  actor: ActorRef<MapActor>,
  remote: RemoteActorRef<MapActor>,
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

  let maps = spawn_maps(&cli.node_id, map_configs()).await?;
  let peer_discovery =
    spawn_peer_map_discovery(cli.node_id.clone(), cli.nodes.clone(), map_ids(&maps));

  let tcp_monitor =
    TcpConnectionMonitor::spawn(TcpConnectionMonitor::new(Duration::from_secs(180)));

  let assignments = assign_players(&player_configs(), &cli.nodes, cli.seed);
  print_assignments(&cli.node_id, &assignments);

  let local_players: Vec<PlayerConfig> = assignments
    .iter()
    .filter_map(|(player, node_id)| (node_id == &cli.node_id).then_some(*player))
    .collect();

  let mut players = spawn_local_players(&cli.node_id, local_players, &maps, &tcp_monitor).await?;

  for runtime in &players {
    let map = find_map(&maps, &runtime.map_id).expect("player map exists");
    let join_info = runtime
      .actor
      .ask(EnterMap {
        map: map.remote.clone(),
      })
      .await?;
    println!(
      "[node:{}] player:{} joined map:{}; own state: {:?} | {} other(s): {:?}",
      cli.node_id,
      runtime.id,
      join_info.map_id,
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
    let map = find_map(&maps, &first_player.map_id).expect("player map exists");
    tokio::time::sleep(Duration::from_millis(200)).await;
    let buff = PlayerBuff::new(
      "manual-stack-training",
      Damage { red: 4, blue: 2 },
      Damage { red: 3, blue: 6 },
    );
    let buff_report = map
      .actor
      .ask(GivePlayerBuff {
        player_id: first_player.id,
        buff,
      })
      .await?
      .expect("local player exists in map");
    println!("[node:{}] buff report: {buff_report:?}", cli.node_id);

    tokio::time::sleep(Duration::from_millis(100)).await;
    let target_id = find_hit_target(&players, &first_player.map_id, first_player.id);
    let hit = map
      .actor
      .ask(HitPlayer {
        attacker_id: first_player.id,
        target_id,
        bonus_damage: Damage { red: 1, blue: 1 },
      })
      .await?
      .expect("attacker and target exist in map");
    println!("[node:{}] hit report: {hit:?}", cli.node_id);

    tokio::time::sleep(Duration::from_millis(100)).await;
    let target_player = players
      .iter()
      .find(|player| player.id == hit.target_id)
      .expect("hit target has local runtime");
    let view = target_player.actor.ask(GetPlayerView).await?;
    let mirror = view.map_mirror.expect("hit player has map mirror");
    assert_eq!(mirror.own_state.stats.red, hit.state_after_hit.stats.red);
    assert_eq!(mirror.own_state.stats.blue, hit.state_after_hit.stats.blue);
    assert_eq!(
      buff_report.state_after_buff.buffs.len(),
      first_player
        .actor
        .ask(GetPlayerView)
        .await?
        .map_mirror
        .unwrap()
        .own_state
        .buffs
        .len()
    );
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

  for map in &maps {
    let all_players = map.actor.ask(GetAllPlayers).await?;
    println!(
      "[node:{}] local map:{} authoritative snapshot: {all_players:?}",
      cli.node_id, map.id
    );
  }

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
    shutdown(maps, tcp_monitor, players).await?;
    return Ok(());
  }

  println!(
    "[node:{}] running in background actor mode. Press Ctrl-C to shut down.",
    cli.node_id
  );
  tokio::signal::ctrl_c().await?;
  println!("[node:{}] shutting down ...", cli.node_id);

  abort_peer_discovery(peer_discovery);
  shutdown(maps, tcp_monitor, players).await?;
  Ok(())
}

async fn spawn_maps(
  node_id: &str,
  configs: Vec<MapConfig>,
) -> Result<Vec<MapRuntime>, Box<dyn std::error::Error>> {
  let mut maps = Vec::new();

  for config in configs {
    let map_id = config.id.clone();
    let map_name = format!("map:{node_id}:{map_id}");
    let map = MapActor::spawn(MapActor::new(config));
    map.register(map_name.as_str()).await?;
    let remote = map.into_remote_ref().await;
    println!("[node:{node_id}] MapActor registered as \"{map_name}\"");
    maps.push(MapRuntime {
      id: map_id,
      actor: map,
      remote,
    });
  }

  Ok(maps)
}

fn map_ids(maps: &[MapRuntime]) -> Vec<String> {
  maps.iter().map(|map| map.id.clone()).collect()
}

fn spawn_peer_map_discovery(
  node_id: String,
  nodes: Vec<String>,
  map_ids: Vec<String>,
) -> Option<JoinHandle<()>> {
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
        for map_id in &map_ids {
          let map_name = format!("map:{peer_node}:{map_id}");
          match RemoteActorRef::<MapActor>::lookup(map_name.as_str()).await {
            Ok(Some(peer_map)) => match peer_map.ask(&GetAllPlayers).await {
              Ok(players) => {
                logged_errors.remove(&map_name);
                if connected.insert(map_name.clone()) {
                  println!(
                    "[node:{node_id}] connected to peer map \"{map_name}\" ({} player(s))",
                    players.len()
                  );
                }
              }
              Err(err) => {
                if connected.remove(&map_name) {
                  println!("[node:{node_id}] lost peer map \"{map_name}\"");
                }
                if logged_errors.insert(map_name.clone()) {
                  eprintln!("[node:{node_id}] peer map ask failed for \"{map_name}\": {err}");
                }
              }
            },
            Ok(None) => {
              logged_errors.remove(&map_name);
              if connected.remove(&map_name) {
                println!("[node:{node_id}] lost peer map \"{map_name}\"");
              }
            }
            Err(err) => {
              if connected.remove(&map_name) {
                println!("[node:{node_id}] lost peer map \"{map_name}\"");
              }
              if logged_errors.insert(map_name.clone()) {
                eprintln!("[node:{node_id}] peer map lookup failed for \"{map_name}\": {err}");
              }
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
      base_attack: Damage { red: 8, blue: 2 },
      base_damage: Damage { red: 6, blue: 1 },
    },
    PlayerConfig {
      id: 1002,
      name: "Mage",
      red: 60,
      blue: 150,
      base_attack: Damage { red: 2, blue: 9 },
      base_damage: Damage { red: 1, blue: 7 },
    },
    PlayerConfig {
      id: 1003,
      name: "Archer",
      red: 85,
      blue: 95,
      base_attack: Damage { red: 7, blue: 4 },
      base_damage: Damage { red: 4, blue: 3 },
    },
    PlayerConfig {
      id: 1004,
      name: "Cleric",
      red: 75,
      blue: 130,
      base_attack: Damage { red: 3, blue: 6 },
      base_damage: Damage { red: 2, blue: 5 },
    },
    PlayerConfig {
      id: 1005,
      name: "Rogue",
      red: 90,
      blue: 70,
      base_attack: Damage { red: 9, blue: 1 },
      base_damage: Damage { red: 5, blue: 2 },
    },
  ]
}

fn map_configs() -> Vec<MapConfig> {
  vec![
    MapConfig::new(
      "green-fields",
      vec![PlayerBuff::new(
        "green-fields-aura",
        Damage { red: 2, blue: 1 },
        Damage { red: 1, blue: 0 },
      )],
    ),
    MapConfig::new(
      "crystal-cave",
      vec![PlayerBuff::new(
        "crystal-focus",
        Damage { red: 0, blue: 4 },
        Damage { red: 1, blue: 3 },
      )],
    ),
    MapConfig::new(
      "ember-keep",
      vec![PlayerBuff::new(
        "ember-rage",
        Damage { red: 5, blue: 0 },
        Damage { red: 4, blue: 1 },
      )],
    ),
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
  maps: &[MapRuntime],
  tcp_monitor: &ActorRef<TcpConnectionMonitor>,
) -> Result<Vec<PlayerRuntime>, Box<dyn std::error::Error>> {
  let mut runtimes = Vec::new();
  for config in players {
    let map_id = assign_player_map(config.id, maps).to_string();
    let player = PlayerActor::spawn(PlayerActor {
      profile: PlayerProfile {
        id: config.id,
        name: config.name.to_string(),
      },
      initial_map_stats: InitialMapStats {
        red: config.red,
        blue: config.blue,
      },
      initial_combat_stats: InitialCombatStats {
        base_attack: config.base_attack,
        base_damage: config.base_damage,
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
      map_id,
      actor: player,
      connection_id,
    });
  }
  Ok(runtimes)
}

fn assign_player_map(player_id: PlayerId, maps: &[MapRuntime]) -> &str {
  let index = player_id as usize % maps.len();
  maps[index].id.as_str()
}

fn find_map<'a>(maps: &'a [MapRuntime], map_id: &str) -> Option<&'a MapRuntime> {
  maps.iter().find(|map| map.id == map_id)
}

fn find_hit_target(players: &[PlayerRuntime], map_id: &str, attacker_id: PlayerId) -> PlayerId {
  players
    .iter()
    .find(|player| player.map_id == map_id && player.id != attacker_id)
    .map_or(attacker_id, |player| player.id)
}

async fn shutdown(
  maps: Vec<MapRuntime>,
  tcp_monitor: ActorRef<TcpConnectionMonitor>,
  players: Vec<PlayerRuntime>,
) -> Result<(), Box<dyn std::error::Error>> {
  tcp_monitor.stop_gracefully().await?;
  tcp_monitor.wait_for_shutdown().await;

  for runtime in players {
    runtime.actor.stop_gracefully().await?;
    runtime.actor.wait_for_shutdown().await;
  }

  for map in maps {
    map.actor.stop_gracefully().await?;
    map.actor.wait_for_shutdown().await;
  }
  Ok(())
}
