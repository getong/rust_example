use std::{collections::HashMap, thread, time::Duration};

use avian3d::prelude::{Gravity, PhysicsPlugins};
use bevior_tree::prelude::{BehaviorTreePlugin, BehaviorTreeSystemSet};
use bevy::{
  app::ScheduleRunnerPlugin, ecs::schedule::ApplyDeferred, prelude::*, transform::TransformPlugin,
};
use tokio::sync::mpsc;

use crate::{
  behavior,
  game::{
    self, ActorId, ActorPresentation, ActorType, ArenaPosition, NextActorId, PlayerInputState,
    ServerTick, SnapshotClock, Vitals, actor_state, spawn_player,
  },
  protocol::{
    ClientEnvelope, ServerEnvelope, Welcome, WorldSnapshot, client_envelope, server_envelope,
  },
  routing::{GatewayEvent, SHARD_COMMAND_BUFFER, ShardCommand, ShardHandle},
  terrain::{self, LevelMap, map_state},
};

pub(crate) const DEFAULT_SHARD_COUNT: usize = 4;

const SERVER_TICK_SECONDS: f64 = 1.0 / 30.0;
const MAX_SHARD_COMMANDS_PER_TICK: usize = 4_096;
const SHARD_ACTOR_ID_STRIDE: u64 = 1_000_000_000;

#[derive(Resource, Clone, Copy)]
struct ShardInfo {
  id: usize,
}

#[derive(Resource)]
struct ShardInbox {
  receiver: mpsc::Receiver<ShardCommand>,
}

#[derive(Resource, Clone)]
struct GatewaySender {
  sender: mpsc::Sender<GatewayEvent>,
}

#[derive(Resource, Default)]
struct ShardClients {
  clients: HashMap<u64, ShardClientState>,
}

struct ShardClientState {
  player: Option<Entity>,
  actor_id: Option<ActorId>,
  name: String,
}

pub(crate) fn spawn_shards(
  count: usize,
  gateway_sender: mpsc::Sender<GatewayEvent>,
) -> Vec<ShardHandle> {
  let count = count.max(1);
  let mut shards = Vec::with_capacity(count);

  for id in 0 .. count {
    let (sender, receiver) = mpsc::channel(SHARD_COMMAND_BUFFER);
    let shard_gateway_sender = gateway_sender.clone();

    thread::Builder::new()
      .name(format!("game-shard-{id}"))
      .spawn(move || run_shard_app(id, receiver, shard_gateway_sender))
      .expect("game shard thread should start");

    shards.push(ShardHandle { id, sender });
  }

  shards
}

fn run_shard_app(
  shard_id: usize,
  receiver: mpsc::Receiver<ShardCommand>,
  gateway_sender: mpsc::Sender<GatewayEvent>,
) {
  App::new()
    .add_plugins((
      MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        SERVER_TICK_SECONDS,
      ))),
      TransformPlugin,
      PhysicsPlugins::default(),
      BehaviorTreePlugin::default().in_schedule(Update),
    ))
    .insert_resource(Gravity::ZERO)
    .insert_resource(ShardInfo { id: shard_id })
    .insert_resource(ShardInbox { receiver })
    .insert_resource(GatewaySender {
      sender: gateway_sender,
    })
    .insert_resource(NextActorId::new(actor_id_base(shard_id)))
    .init_resource::<game::ServerTick>()
    .init_resource::<game::SnapshotClock>()
    .init_resource::<game::CombatClock>()
    .init_resource::<terrain::LevelMap>()
    .init_resource::<terrain::TerrainMap>()
    .init_resource::<ShardClients>()
    .add_systems(
      Startup,
      (terrain::spawn_static_colliders, game::spawn_monsters).chain(),
    )
    .add_systems(
      Update,
      (
        drain_shard_commands.before(BehaviorTreeSystemSet::Update),
        ApplyDeferred
          .after(drain_shard_commands)
          .before(BehaviorTreeSystemSet::Update),
        game::apply_player_movement.before(BehaviorTreeSystemSet::Update),
        behavior::move_chasing_monsters.after(BehaviorTreeSystemSet::Update),
        game::sync_actor_transforms.after(behavior::move_chasing_monsters),
        game::update_actor_presentation.after(game::sync_actor_transforms),
        game::resolve_combat.after(game::update_actor_presentation),
        broadcast_shard_snapshots.after(game::resolve_combat),
      ),
    )
    .run();
}

fn actor_id_base(shard_id: usize) -> u64 {
  1 + shard_id as u64 * SHARD_ACTOR_ID_STRIDE
}

fn drain_shard_commands(
  mut commands: Commands,
  mut inbox: ResMut<ShardInbox>,
  mut clients: ResMut<ShardClients>,
  mut actor_ids: ResMut<NextActorId>,
  level_map: Res<LevelMap>,
  shard: Res<ShardInfo>,
  gateway: Res<GatewaySender>,
  tick: Res<ServerTick>,
  mut player_inputs: Query<&mut PlayerInputState>,
) {
  for _ in 0 .. MAX_SHARD_COMMANDS_PER_TICK {
    let Ok(command) = inbox.receiver.try_recv() else {
      break;
    };

    match command {
      ShardCommand::Connected { client_id } => {
        clients.clients.insert(
          client_id,
          ShardClientState {
            player: None,
            actor_id: None,
            name: format!("Player {client_id}"),
          },
        );
      }
      ShardCommand::Message { client_id, message } => {
        handle_client_message(
          &mut commands,
          &mut clients,
          &mut actor_ids,
          &level_map,
          &gateway,
          &mut player_inputs,
          tick.0,
          shard.id,
          client_id,
          message,
        );
      }
      ShardCommand::Disconnected { client_id } => {
        if let Some(client) = clients.clients.remove(&client_id)
          && let Some(player) = client.player
        {
          commands.entity(player).despawn();
        }
      }
    }
  }
}

fn broadcast_shard_snapshots(
  time: Res<Time>,
  tick: Res<ServerTick>,
  mut clock: ResMut<SnapshotClock>,
  level_map: Res<LevelMap>,
  gateway: Res<GatewaySender>,
  clients: Res<ShardClients>,
  actors: Query<(
    &ActorId,
    &ActorType,
    &ArenaPosition,
    &Vitals,
    &ActorPresentation,
  )>,
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

  for client_id in clients.clients.keys().copied() {
    send_to_client(&gateway, client_id, envelope.clone());
  }
}

fn handle_client_message(
  commands: &mut Commands,
  clients: &mut ShardClients,
  actor_ids: &mut NextActorId,
  level_map: &LevelMap,
  gateway: &GatewaySender,
  player_inputs: &mut Query<&mut PlayerInputState>,
  tick: u64,
  shard_id: usize,
  client_id: u64,
  message: ClientEnvelope,
) {
  let connected_players = clients
    .clients
    .values()
    .filter(|client| client.player.is_some())
    .count();
  let Some(client) = clients.clients.get_mut(&client_id) else {
    return;
  };

  match message.payload {
    Some(client_envelope::Payload::Hello(hello)) => {
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
        send_to_client(gateway, client_id, welcome);
      }
    }
    Some(client_envelope::Payload::Input(input)) => {
      let Some(player) = client.player else {
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
      send_to_client(gateway, client_id, pong);
    }
    None => {
      let notice = ServerEnvelope {
        payload: Some(server_envelope::Payload::Notice(crate::protocol::Notice {
          message: format!("shard {shard_id} ignored empty client message"),
        })),
      };
      send_to_client(gateway, client_id, notice);
    }
  }
}

fn send_to_client(gateway: &GatewaySender, client_id: u64, message: ServerEnvelope) {
  let _ = gateway
    .sender
    .try_send(GatewayEvent::Send { client_id, message });
}
