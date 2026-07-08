use std::{collections::HashMap, net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use avian3d::prelude::{Gravity, PhysicsPlugins};
use bevior_tree::prelude::{BehaviorTreePlugin, BehaviorTreeSystemSet};
use bevy::{
  app::ScheduleRunnerPlugin, ecs::schedule::ApplyDeferred, log::LogPlugin, prelude::*,
  state::app::StatesPlugin, transform::TransformPlugin,
};
use lightyear::prelude::{
  Authentication, Connected, Disconnected, LinkOf, LocalAddr, MessageReceiver, MessageSender,
  RemoteId, ReplicationMetadata, ReplicationReceiver, ReplicationSender, UdpIo,
  client::{ClientPlugins, Connect, NetcodeClient, NetcodeConfig as ClientNetcodeConfig},
  server::{
    ClientOf, NetcodeConfig as ServerNetcodeConfig, NetcodeServer, ServerPlugins, ServerUdpIo,
    Start,
  },
};

use crate::{
  agones::AgonesGameServerInfo,
  behavior, game,
  game::{
    ActorId, ActorPresentation, ActorType, ArenaPosition, NextActorId, PlayerInputState,
    ServerTick, SnapshotClock, Vitals, actor_state, spawn_player,
  },
  player_registry::{self, PlayerServerRegistry},
  protocol::{
    ClientEnvelope, ClientPacket, GameChannel, GameProtocolPlugin, NETCODE_PRIVATE_KEY,
    NETCODE_PROTOCOL_ID, ServerEnvelope, ServerPacket, Welcome, WorldSnapshot, client_envelope,
    decode_client_packet, encode_server_envelope, server_envelope,
  },
  replication, terrain,
  terrain::{LevelMap, map_state},
};

const SERVER_TICK_SECONDS: f64 = 1.0 / 30.0;
const REPLICATION_SEND_SECONDS: f64 = 0.1;
const MAX_CLIENT_MESSAGES_PER_TICK: usize = 4_096;

#[derive(Resource)]
struct ServerConfig {
  bind_addr: SocketAddr,
}

#[derive(Resource, Default)]
struct ServerClients {
  clients: HashMap<u64, ServerClientState>,
}

struct ServerClientState {
  entity: Entity,
  player: Option<Entity>,
  actor_id: Option<ActorId>,
  name: String,
}

pub(crate) fn run_server(bind_addr: &str) -> Result<()> {
  let bind_addr = bind_addr
    .parse::<SocketAddr>()
    .with_context(|| format!("invalid game server bind address {bind_addr}"))?;

  println!("game_server lightyear ecs server listening on {bind_addr}, replication=bevy_replicon");

  App::new()
    .add_plugins((
      MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        SERVER_TICK_SECONDS,
      ))),
      LogPlugin::default(),
      StatesPlugin,
      TransformPlugin,
      PhysicsPlugins::default(),
      BehaviorTreePlugin::default().in_schedule(Update),
    ))
    .add_plugins(ClientPlugins {
      tick_duration: Duration::from_secs_f64(SERVER_TICK_SECONDS),
    })
    .add_plugins(ServerPlugins {
      tick_duration: Duration::from_secs_f64(SERVER_TICK_SECONDS),
    })
    .add_plugins(crate::agones::AgonesPlugin)
    .add_plugins(player_registry::PlayerServerRegistryPlugin)
    .add_plugins(GameProtocolPlugin)
    .add_plugins(replication::GameReplicationPlugin)
    .insert_resource(Gravity::ZERO)
    .insert_resource(ReplicationMetadata::new(Duration::from_secs_f64(
      REPLICATION_SEND_SECONDS,
    )))
    .insert_resource(ServerConfig { bind_addr })
    .init_resource::<NextActorId>()
    .init_resource::<ServerTick>()
    .init_resource::<SnapshotClock>()
    .init_resource::<game::CombatClock>()
    .init_resource::<terrain::LevelMap>()
    .init_resource::<terrain::TerrainMap>()
    .init_resource::<ServerClients>()
    .add_systems(
      Startup,
      (
        start_lightyear_server,
        start_replication_peers,
        (terrain::spawn_static_colliders, game::spawn_monsters).chain(),
      ),
    )
    .add_systems(
      Update,
      (
        drain_client_messages.before(BehaviorTreeSystemSet::Update),
        ApplyDeferred
          .after(drain_client_messages)
          .before(BehaviorTreeSystemSet::Update),
        replication::mark_replicated_actors
          .after(ApplyDeferred)
          .before(BehaviorTreeSystemSet::Update),
        game::apply_player_movement.before(BehaviorTreeSystemSet::Update),
        behavior::move_chasing_monsters.after(BehaviorTreeSystemSet::Update),
        game::sync_actor_transforms.after(behavior::move_chasing_monsters),
        game::update_actor_presentation.after(game::sync_actor_transforms),
        game::resolve_combat.after(game::update_actor_presentation),
        replication::sync_replicated_actor_state.after(game::resolve_combat),
        broadcast_snapshots.after(replication::sync_replicated_actor_state),
      ),
    )
    .add_observer(enable_replication_for_client)
    .add_observer(connect_client)
    .add_observer(disconnect_client)
    .run();

  Ok(())
}

fn start_lightyear_server(mut commands: Commands, config: Res<ServerConfig>) {
  let server = commands
    .spawn((
      ServerUdpIo::default(),
      NetcodeServer::new(
        ServerNetcodeConfig::default()
          .with_protocol_id(NETCODE_PROTOCOL_ID)
          .with_key(NETCODE_PRIVATE_KEY),
      ),
      LocalAddr(config.bind_addr),
    ))
    .id();
  commands.trigger(Start { entity: server });
}

fn start_replication_peers(mut commands: Commands) {
  for (index, peer_addr) in replication_peer_addrs().into_iter().enumerate() {
    let client_id = replication_peer_client_id(index, peer_addr);
    let auth = Authentication::Manual {
      server_addr: peer_addr,
      client_id,
      private_key: NETCODE_PRIVATE_KEY,
      protocol_id: NETCODE_PROTOCOL_ID,
    };
    let netcode_client = match NetcodeClient::new(auth, ClientNetcodeConfig::default()) {
      Ok(client) => client,
      Err(err) => {
        eprintln!("game_server replication peer {peer_addr} init failed: {err}");
        continue;
      }
    };
    let entity = commands
      .spawn((
        UdpIo::default(),
        LocalAddr(replication_peer_local_addr(peer_addr)),
        ReplicationReceiver,
        netcode_client,
      ))
      .id();
    println!("game_server replication peer connecting to {peer_addr} as {client_id}");
    commands.trigger(Connect { entity });
  }
}

fn enable_replication_for_client(trigger: On<Add, LinkOf>, mut commands: Commands) {
  commands.entity(trigger.entity).insert(ReplicationSender);
}

fn connect_client(trigger: On<Add, Connected>, clients: Query<&RemoteId, With<ClientOf>>) {
  let Ok(remote_id) = clients.get(trigger.entity) else {
    return;
  };
  let Some(client_id) = netcode_client_id(remote_id) else {
    return;
  };
  println!("client {client_id} connected");
}

fn drain_client_messages(
  mut commands: Commands,
  mut clients: ResMut<ServerClients>,
  mut actor_ids: ResMut<NextActorId>,
  config: Res<ServerConfig>,
  agones_info: Res<AgonesGameServerInfo>,
  player_registry: Res<PlayerServerRegistry>,
  level_map: Res<LevelMap>,
  tick: Res<ServerTick>,
  mut client_query: Query<
    (
      Entity,
      &RemoteId,
      &mut MessageReceiver<ClientPacket>,
      Option<&Connected>,
    ),
    With<ClientOf>,
  >,
  mut player_inputs: Query<&mut PlayerInputState>,
  mut senders: Query<&mut MessageSender<ServerPacket>, With<ClientOf>>,
) {
  let mut processed = 0usize;

  'clients: for (entity, remote_id, mut receiver, connected) in &mut client_query {
    if connected.is_none() {
      continue;
    }

    let Some(client_id) = netcode_client_id(remote_id) else {
      continue;
    };

    for packet in receiver.receive() {
      if processed >= MAX_CLIENT_MESSAGES_PER_TICK {
        break 'clients;
      }
      processed += 1;

      match decode_client_packet(packet) {
        Ok(message) => handle_client_message(
          &mut commands,
          &mut clients,
          &mut actor_ids,
          &level_map,
          &mut player_inputs,
          &mut senders,
          config.bind_addr,
          &agones_info,
          &player_registry,
          tick.0,
          client_id,
          entity,
          message,
        ),
        Err(err) => eprintln!("client {client_id} message decode error: {err:#}"),
      }
    }
  }
}

fn disconnect_client(
  trigger: On<Add, Disconnected>,
  mut commands: Commands,
  mut clients: ResMut<ServerClients>,
  player_registry: Res<PlayerServerRegistry>,
  client_query: Query<&RemoteId, With<ClientOf>>,
) {
  let Ok(remote_id) = client_query.get(trigger.entity) else {
    return;
  };
  let Some(client_id) = netcode_client_id(remote_id) else {
    return;
  };
  println!("client {client_id} disconnected");
  player_registry.remove(client_id);
  if let Some(client) = clients.clients.remove(&client_id)
    && let Some(player) = client.player
  {
    commands.entity(player).despawn();
  }
}

fn broadcast_snapshots(
  time: Res<Time>,
  tick: Res<ServerTick>,
  mut clock: ResMut<SnapshotClock>,
  level_map: Res<LevelMap>,
  clients: Res<ServerClients>,
  actors: Query<(
    &ActorId,
    &ActorType,
    &ArenaPosition,
    &Vitals,
    &ActorPresentation,
  )>,
  mut senders: Query<&mut MessageSender<ServerPacket>, With<ClientOf>>,
) {
  clock.0.tick(time.delta());
  if !clock.0.just_finished() {
    return;
  }

  let snapshot = WorldSnapshot {
    tick: tick.0,
    actors: actors
      .iter()
      .map(|(id, kind, position, vitals, presentation)| {
        actor_state(*id, *kind, *position, *vitals, *presentation)
      })
      .collect(),
    map: Some(map_state(&level_map)),
  };
  let envelope = ServerEnvelope {
    payload: Some(server_envelope::Payload::Snapshot(snapshot)),
  };

  for (client_id, client) in &clients.clients {
    send_to_client(&mut senders, client.entity, *client_id, envelope.clone());
  }
}

fn handle_client_message(
  commands: &mut Commands,
  clients: &mut ServerClients,
  actor_ids: &mut NextActorId,
  level_map: &LevelMap,
  player_inputs: &mut Query<&mut PlayerInputState>,
  senders: &mut Query<&mut MessageSender<ServerPacket>, With<ClientOf>>,
  bind_addr: SocketAddr,
  agones_info: &AgonesGameServerInfo,
  player_registry: &PlayerServerRegistry,
  tick: u64,
  client_id: u64,
  client_entity: Entity,
  message: ClientEnvelope,
) {
  match message.payload {
    Some(client_envelope::Payload::Hello(hello)) => {
      let room = hello.room.trim().to_string();
      let connected_players = clients
        .clients
        .values()
        .filter(|client| client.player.is_some())
        .count();
      let client = clients
        .clients
        .entry(client_id)
        .or_insert_with(|| ServerClientState {
          entity: client_entity,
          player: None,
          actor_id: None,
          name: format!("Player {client_id}"),
        });
      client.entity = client_entity;
      client.name = hello.name;

      if client.player.is_none() {
        let actor_id = actor_ids.next();
        let player = spawn_player(commands, actor_id, client_id, connected_players, level_map);
        client.player = Some(player);
        client.actor_id = Some(actor_id);

        let welcome = ServerEnvelope {
          payload: Some(server_envelope::Payload::Welcome(Welcome {
            client_id,
            actor_id: actor_id.0,
            tick,
          })),
        };
        send_to_client(senders, client_entity, client_id, welcome);
      }

      let record = player_registry::player_server_record(
        client_id,
        Some(room.as_str()),
        agones_info,
        bind_addr,
      );
      player_registry.upsert(record);
    }
    Some(client_envelope::Payload::Input(input)) => {
      let player = clients
        .clients
        .get(&client_id)
        .and_then(|client| client.player);
      let Some(player) = player else {
        return;
      };
      let Ok(mut player_input) = player_inputs.get_mut(player) else {
        return;
      };

      player_input.direction = Vec3::new(input.x, 0.0, input.z).clamp_length_max(1.0);
    }
    Some(client_envelope::Payload::Ping(ping)) => {
      let pong = ServerEnvelope {
        payload: Some(server_envelope::Payload::Pong(crate::protocol::Pong {
          client_time_ms: ping.client_time_ms,
          server_tick: tick,
        })),
      };
      send_to_client(senders, client_entity, client_id, pong);
    }
    None => {
      let notice = ServerEnvelope {
        payload: Some(server_envelope::Payload::Notice(crate::protocol::Notice {
          message: "ignored empty client message".to_string(),
        })),
      };
      send_to_client(senders, client_entity, client_id, notice);
    }
  }
}

fn send_to_client(
  senders: &mut Query<&mut MessageSender<ServerPacket>, With<ClientOf>>,
  client_entity: Entity,
  client_id: u64,
  message: ServerEnvelope,
) {
  let Ok(mut sender) = senders.get_mut(client_entity) else {
    return;
  };

  match encode_server_envelope(&message) {
    Ok(packet) => sender.send::<GameChannel>(packet),
    Err(err) => eprintln!("client {client_id} message encode error: {err:#}"),
  }
}

fn netcode_client_id(remote_id: &RemoteId) -> Option<u64> {
  match remote_id.0 {
    lightyear::prelude::PeerId::Netcode(client_id) => Some(client_id),
    _ => None,
  }
}

fn replication_peer_addrs() -> Vec<SocketAddr> {
  let Ok(value) = std::env::var("GAME_SERVER_REPLICATION_PEERS") else {
    return Vec::new();
  };

  value
    .split(|ch| matches!(ch, ',' | ';' | ' ' | '\n' | '\t'))
    .filter_map(|raw_addr| {
      let raw_addr = raw_addr.trim();
      if raw_addr.is_empty() {
        return None;
      }
      match raw_addr.parse::<SocketAddr>() {
        Ok(addr) => Some(addr),
        Err(err) => {
          eprintln!("game_server ignored invalid replication peer {raw_addr}: {err}");
          None
        }
      }
    })
    .collect()
}

fn replication_peer_local_addr(peer_addr: SocketAddr) -> SocketAddr {
  if peer_addr.is_ipv4() {
    SocketAddr::from(([0, 0, 0, 0], 0))
  } else {
    SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], 0))
  }
}

fn replication_peer_client_id(index: usize, peer_addr: SocketAddr) -> u64 {
  let identity = std::env::var("GAME_SERVER_REPLICATION_ID")
    .or_else(|_| std::env::var("HOSTNAME"))
    .unwrap_or_else(|_| format!("pid-{}", std::process::id()));
  let client_id =
    stable_hash(format!("game-server-peer:{identity}:{index}:{peer_addr}").as_bytes());
  client_id.max(1)
}

fn stable_hash(bytes: &[u8]) -> u64 {
  const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

  bytes.iter().fold(FNV_OFFSET, |hash, byte| {
    (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
  })
}
