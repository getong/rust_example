use std::{collections::HashMap, net::SocketAddr, time::Duration};

use anyhow::{Context, Result, bail};
use bevy::{app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*};
use lightyear::prelude::{
  Connected, Disconnected, LocalAddr, MessageReceiver, MessageSender, PeerId, RemoteId,
  server::{ClientOf, NetcodeConfig, NetcodeServer, ServerPlugins, ServerUdpIo, Start},
};
use tokio::sync::mpsc;

use crate::{
  protocol::{
    ClientEnvelope, ClientPacket, GameChannel, GameProtocolPlugin, NETCODE_PRIVATE_KEY,
    NETCODE_PROTOCOL_ID, ServerPacket, client_envelope, decode_client_packet,
    encode_server_envelope,
  },
  routing::{GatewayEvent, ShardCommand, ShardHandle},
};

const GATEWAY_TICK_SECONDS: f64 = 1.0 / 60.0;
const MAX_GATEWAY_EVENTS_PER_TICK: usize = 16_384;

#[derive(Resource)]
struct GatewayConfig {
  bind_addr: SocketAddr,
}

#[derive(Resource)]
struct GatewayState {
  shards: Vec<ShardHandle>,
  gateway_receiver: mpsc::Receiver<GatewayEvent>,
  clients: HashMap<u64, ClientConnection>,
}

struct ClientConnection {
  shard_id: usize,
  entity: Entity,
}

pub(crate) fn run_gateway(
  bind_addr: &str,
  shards: Vec<ShardHandle>,
  gateway_receiver: mpsc::Receiver<GatewayEvent>,
) -> Result<()> {
  if shards.is_empty() {
    bail!("gateway requires at least one shard");
  }

  let bind_addr = bind_addr
    .parse::<SocketAddr>()
    .with_context(|| format!("invalid gateway bind address {bind_addr}"))?;

  println!(
    "game_server lightyear gateway listening on {bind_addr}, shards={}",
    shards.len()
  );

  App::new()
    .add_plugins(
      MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
        GATEWAY_TICK_SECONDS,
      ))),
    )
    .add_plugins(LogPlugin::default())
    .add_plugins(ServerPlugins {
      tick_duration: Duration::from_secs_f64(GATEWAY_TICK_SECONDS),
    })
    .add_plugins(crate::agones::AgonesPlugin)
    .add_plugins(GameProtocolPlugin)
    .insert_resource(GatewayConfig { bind_addr })
    .insert_resource(GatewayState {
      shards,
      gateway_receiver,
      clients: HashMap::new(),
    })
    .add_systems(Startup, start_lightyear_server)
    .add_systems(
      Update,
      (drain_client_messages, drain_gateway_events).chain(),
    )
    .add_observer(connect_client)
    .add_observer(disconnect_client)
    .run();

  Ok(())
}

fn start_lightyear_server(mut commands: Commands, config: Res<GatewayConfig>) {
  let server = commands
    .spawn((
      ServerUdpIo::default(),
      NetcodeServer::new(
        NetcodeConfig::default()
          .with_protocol_id(NETCODE_PROTOCOL_ID)
          .with_key(NETCODE_PRIVATE_KEY),
      ),
      LocalAddr(config.bind_addr),
    ))
    .id();
  commands.trigger(Start { entity: server });
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
  mut state: ResMut<GatewayState>,
  mut clients: Query<
    (
      Entity,
      &RemoteId,
      &mut MessageReceiver<ClientPacket>,
      Option<&Connected>,
    ),
    With<ClientOf>,
  >,
) {
  for (entity, remote_id, mut receiver, connected) in &mut clients {
    if connected.is_none() {
      continue;
    }

    let Some(client_id) = netcode_client_id(remote_id) else {
      continue;
    };

    for packet in receiver.receive() {
      match decode_client_packet(packet) {
        Ok(message) => route_client_message(&mut state, client_id, entity, message),
        Err(err) => eprintln!("client {client_id} message decode error: {err:#}"),
      }
    }
  }
}

fn drain_gateway_events(
  mut state: ResMut<GatewayState>,
  mut senders: Query<&mut MessageSender<ServerPacket>, With<ClientOf>>,
) {
  for _ in 0 .. MAX_GATEWAY_EVENTS_PER_TICK {
    let Ok(event) = state.gateway_receiver.try_recv() else {
      break;
    };

    match event {
      GatewayEvent::Send { client_id, message } => {
        let Some(entity) = state.clients.get(&client_id).map(|client| client.entity) else {
          continue;
        };
        let Ok(mut sender) = senders.get_mut(entity) else {
          state.clients.remove(&client_id);
          continue;
        };
        match encode_server_envelope(&message) {
          Ok(packet) => sender.send::<GameChannel>(packet),
          Err(err) => eprintln!("client {client_id} message encode error: {err:#}"),
        }
      }
    }
  }
}

fn disconnect_client(
  trigger: On<Add, Disconnected>,
  mut state: ResMut<GatewayState>,
  clients: Query<&RemoteId, With<ClientOf>>,
) {
  let Ok(remote_id) = clients.get(trigger.entity) else {
    return;
  };
  let Some(client_id) = netcode_client_id(remote_id) else {
    return;
  };
  println!("client {client_id} disconnected");
  disconnect_client_by_id(&mut state, client_id);
}

fn route_client_message(
  state: &mut GatewayState,
  client_id: u64,
  entity: Entity,
  message: ClientEnvelope,
) {
  if let Some(client) = state.clients.get(&client_id) {
    if let Some(shard) = state.shards.get(client.shard_id) {
      send_to_shard(&shard.sender, ShardCommand::Message { client_id, message });
    }
    return;
  }

  let Ok(shard) = choose_shard(client_id, &message, &state.shards) else {
    return;
  };
  let shard_id = shard.id;
  let shard_sender = shard.sender.clone();
  state
    .clients
    .insert(client_id, ClientConnection { shard_id, entity });
  send_to_shard(&shard_sender, ShardCommand::Connected { client_id });
  send_to_shard(&shard_sender, ShardCommand::Message { client_id, message });
}

fn disconnect_client_by_id(state: &mut GatewayState, client_id: u64) {
  let Some(client) = state.clients.remove(&client_id) else {
    return;
  };
  let Some(shard) = state.shards.get(client.shard_id) else {
    return;
  };
  send_to_shard(&shard.sender, ShardCommand::Disconnected { client_id });
}

fn send_to_shard(shard_sender: &mpsc::Sender<ShardCommand>, command: ShardCommand) {
  match shard_sender.try_send(command) {
    Ok(()) => {}
    Err(mpsc::error::TrySendError::Full(_)) => {}
    Err(mpsc::error::TrySendError::Closed(_)) => {
      eprintln!("client shard input channel closed");
    }
  }
}

fn choose_shard<'a>(
  client_id: u64,
  first_message: &ClientEnvelope,
  shards: &'a [ShardHandle],
) -> Result<&'a ShardHandle> {
  let route_key = route_key(client_id, first_message);
  let shard_index = route_key as usize % shards.len();
  shards
    .get(shard_index)
    .context("gateway has no shard for client")
}

fn route_key(client_id: u64, first_message: &ClientEnvelope) -> u64 {
  if let Some(client_envelope::Payload::Hello(hello)) = &first_message.payload {
    let room = hello.room.trim();
    if !room.is_empty() {
      return stable_hash(room.as_bytes());
    }
  }

  client_id
}

fn stable_hash(bytes: &[u8]) -> u64 {
  const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
  const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

  bytes.iter().fold(FNV_OFFSET, |hash, byte| {
    (hash ^ u64::from(*byte)).wrapping_mul(FNV_PRIME)
  })
}

fn netcode_client_id(remote_id: &RemoteId) -> Option<u64> {
  match remote_id.0 {
    PeerId::Netcode(client_id) => Some(client_id),
    _ => None,
  }
}
