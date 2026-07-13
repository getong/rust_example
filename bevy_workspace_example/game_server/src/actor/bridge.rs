//! Bridge between the Bevy world (settlement engine) and the kameo actor
//! layer (player/map processes).
//!
//! Bevy systems stay fully synchronous: they only push [`BridgeOut`] messages
//! into a channel and drain [`WorldCommand`]s once per tick. A pump task on a
//! dedicated tokio runtime does all the async actor messaging, including the
//! optional kameo remote bootstrap (libp2p swarm + distributed registry) that
//! lets player actors and the map actor live on different nodes.

use std::{collections::HashMap, env, sync::Mutex};

use bevy::prelude::*;
use kameo::actor::{ActorRef, RemoteActorRef, Spawn};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
  actor::{
    map::{AdvanceTick, AttachLocalPlayer, MapActor, UpdateVitals},
    player::{EnterMap, LeaveMap, MapLink, PlayerActor},
    types::{CombatBuff, MapVitals, PlayerProfile, WorldCommand},
  },
  game::{self, ActorMapping, CombatModifiers},
};

const DEFAULT_MAP_ID: &str = "arena";
/// How many ticks an early WorldCommand may wait for its player entity to
/// appear before being dropped (spawn happens via Commands, one tick later).
const MAX_COMMAND_RETRIES: u8 = 60;

/// Messages from Bevy systems to the actor pump.
#[derive(Debug)]
pub(crate) enum BridgeOut {
  PlayerHello {
    client_id: u64,
    name: String,
    room: Option<String>,
  },
  PlayerDisconnected {
    client_id: u64,
  },
  VitalsChanged {
    client_id: u64,
    red: i32,
    blue: i32,
  },
  Tick {
    tick: u64,
  },
}

#[derive(Resource)]
pub(crate) struct ActorBridge {
  out_tx: UnboundedSender<BridgeOut>,
  world_rx: Mutex<UnboundedReceiver<WorldCommand>>,
  _runtime: tokio::runtime::Runtime,
}

impl ActorBridge {
  pub(crate) fn send(&self, message: BridgeOut) {
    if self.out_tx.send(message).is_err() {
      eprintln!("game_server actor bridge pump is gone");
    }
  }
}

pub(crate) struct ActorBridgePlugin;

impl Plugin for ActorBridgePlugin {
  fn build(&self, app: &mut App) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
      .worker_threads(2)
      .thread_name("kameo-actor")
      .enable_all()
      .build()
    {
      Ok(runtime) => runtime,
      Err(err) => {
        eprintln!("game_server actor bridge runtime error: {err}");
        return;
      }
    };

    let (out_tx, out_rx) = mpsc::unbounded_channel();
    let (world_tx, world_rx) = mpsc::unbounded_channel();
    runtime.spawn(pump(out_rx, world_tx));

    app
      .insert_resource(ActorBridge {
        out_tx,
        world_rx: Mutex::new(world_rx),
        _runtime: runtime,
      })
      .add_systems(
        Update,
        drain_world_commands.before(game::apply_player_movement),
      );
  }
}

/// Applies actor-layer commands to the ECS. The only writer of
/// [`CombatModifiers`]; the component itself is a derived cache of map-owned
/// buff state.
fn drain_world_commands(
  bridge: Res<ActorBridge>,
  mut pending: Local<Vec<(WorldCommand, u8)>>,
  mut commands: Commands,
  actor_mapping: Res<ActorMapping>,
) {
  let mut queue: Vec<(WorldCommand, u8)> = pending.drain(..).collect();
  if let Ok(mut world_rx) = bridge.world_rx.lock() {
    while let Ok(command) = world_rx.try_recv() {
      queue.push((command, 0));
    }
  }

  for (command, retries) in queue {
    match command {
      WorldCommand::UpdateModifiers {
        player_id,
        effective,
      } => {
        let entity = actor_mapping.lookup(player_id);
        match entity {
          Some(entity) => {
            commands.entity(entity).insert(CombatModifiers {
              damage_taken_mult: effective.damage_taken_mult,
              move_speed_mult: effective.move_speed_mult,
            });
          }
          None if retries < MAX_COMMAND_RETRIES => {
            pending.push((
              WorldCommand::UpdateModifiers {
                player_id,
                effective,
              },
              retries + 1,
            ));
          }
          None => {
            eprintln!("game_server dropped modifiers for missing player {player_id}");
          }
        }
      }
    }
  }
}

/// Async side of the bridge: owns the map actor and one player actor per
/// client, and forwards ECS facts into the actor layer.
async fn pump(mut out_rx: UnboundedReceiver<BridgeOut>, world_tx: UnboundedSender<WorldCommand>) {
  let remote_ready = bootstrap_remote();
  // Registry name of a map actor hosted on another node; when set (and the
  // swarm is up), player processes join that remote map process instead of
  // the local one — the kameo remote registry makes the two cases identical
  // from the player actor's point of view.
  let remote_map_name = env::var("GAME_SERVER_REMOTE_MAP")
    .ok()
    .map(|name| name.trim().to_string())
    .filter(|name| !name.is_empty());

  let map_id = env::var("GAME_SERVER_MAP_ID").unwrap_or_else(|_| DEFAULT_MAP_ID.to_string());
  let map = MapActor::spawn(MapActor::new(
    map_id.clone(),
    map_entry_buffs(),
    Some(world_tx),
  ));
  if remote_ready {
    let map_name = format!("map:{map_id}");
    match map.register(map_name.as_str()).await {
      Ok(()) => println!("game_server map actor registered as \"{map_name}\""),
      Err(err) => eprintln!("game_server map actor register error: {err}"),
    }
  }

  let mut players: HashMap<u64, ActorRef<PlayerActor>> = HashMap::new();

  while let Some(message) = out_rx.recv().await {
    match message {
      BridgeOut::PlayerHello {
        client_id,
        name,
        room,
      } => {
        let registry_name = format!("player:{client_id}");
        let player = match players.get(&client_id) {
          Some(player) => player.clone(),
          None => {
            let player = PlayerActor::spawn(PlayerActor::new(
              PlayerProfile {
                id: client_id,
                name,
                room,
              },
              registry_name.clone(),
            ));
            if remote_ready
              && let Err(err) = player.register(registry_name.as_str()).await
            {
              eprintln!("game_server player actor register error: {err}");
            }
            players.insert(client_id, player.clone());
            player
          }
        };

        let map_link = resolve_map_link(&map, remote_map_name.as_deref(), remote_ready).await;
        let attach_local = matches!(map_link, MapLink::Local(_));
        let join = player
          .ask(EnterMap {
            map: map_link,
            vitals: MapVitals {
              red: game::PLAYER_START_RED,
              blue: game::PLAYER_START_BLUE,
            },
          })
          .await;
        match join {
          Ok(Some(join)) => {
            println!(
              "player {client_id} entered map:{} with {} buff(s)",
              join.map_id,
              join.own.buffs.len()
            );
            if attach_local {
              let _ = map
                .tell(AttachLocalPlayer {
                  player_id: client_id,
                  player,
                })
                .await;
            }
          }
          Ok(None) => eprintln!("player {client_id} map join refused"),
          Err(err) => eprintln!("player {client_id} map join error: {err}"),
        }
      }
      BridgeOut::PlayerDisconnected { client_id } => {
        if let Some(player) = players.get(&client_id) {
          match player.ask(LeaveMap).await {
            Ok(banked) => println!("player {client_id} left map, banked {banked} portable buff(s)"),
            Err(err) => eprintln!("player {client_id} leave map error: {err}"),
          }
        }
        // The actor is kept for reconnects; banked portable buffs survive in
        // its state and are handed to the map on the next EnterMap.
      }
      BridgeOut::VitalsChanged {
        client_id,
        red,
        blue,
      } => {
        let _ = map
          .tell(UpdateVitals {
            player_id: client_id,
            vitals: MapVitals { red, blue },
          })
          .await;
      }
      BridgeOut::Tick { tick } => {
        let _ = map.tell(AdvanceTick { tick }).await;
      }
    }
  }
}

/// Picks the map process a player should join: a remote map actor looked up
/// in the distributed registry when configured and reachable, otherwise the
/// local map actor.
async fn resolve_map_link(
  local_map: &ActorRef<MapActor>,
  remote_map_name: Option<&str>,
  remote_ready: bool,
) -> MapLink {
  if remote_ready && let Some(name) = remote_map_name {
    match RemoteActorRef::<MapActor>::lookup(name).await {
      Ok(Some(remote_map)) => return MapLink::Remote(remote_map),
      Ok(None) => eprintln!("game_server remote map \"{name}\" not registered, using local map"),
      Err(err) => eprintln!("game_server remote map \"{name}\" lookup error: {err}"),
    }
  }
  MapLink::Local(local_map.clone())
}

/// Starts the kameo remote swarm (libp2p, mDNS discovery, distributed actor
/// registry). Disable with GAME_SERVER_KAMEO_REMOTE=off; override the listen
/// multiaddr with GAME_SERVER_KAMEO_LISTEN.
fn bootstrap_remote() -> bool {
  let enabled = env::var("GAME_SERVER_KAMEO_REMOTE")
    .map(|value| {
      !matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "no" | "off"
      )
    })
    .unwrap_or(true);
  if !enabled {
    println!("game_server kameo remote disabled");
    return false;
  }

  let listen =
    env::var("GAME_SERVER_KAMEO_LISTEN").unwrap_or_else(|_| "/ip4/0.0.0.0/tcp/0".to_string());
  match kameo::remote::bootstrap_on(listen.as_str()) {
    Ok(peer_id) => {
      println!("game_server kameo remote peer id: {peer_id}");
      true
    }
    Err(err) => {
      eprintln!("game_server kameo remote bootstrap failed, running local-only: {err}");
      false
    }
  }
}

fn map_entry_buffs() -> Vec<CombatBuff> {
  vec![CombatBuff::permanent("arena-ward", 0.9)]
}
