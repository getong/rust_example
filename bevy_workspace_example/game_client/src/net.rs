use std::{
  net::SocketAddr,
  time::{SystemTime, UNIX_EPOCH},
};

use bevy::prelude::*;
use lightyear::prelude::{
  Authentication, Client, Connected, Disconnected, LocalAddr, MessageReceiver, MessageSender,
  UdpIo,
  client::{Connect, NetcodeClient, NetcodeConfig},
};
use tokio::sync::mpsc;

use crate::{
  ClientWorld,
  protocol::{
    ClientEnvelope, ClientPacket, DEFAULT_SERVER_ADDR, GameChannel, Hello, NETCODE_PRIVATE_KEY,
    NETCODE_PROTOCOL_ID, ServerEnvelope, ServerPacket, client_envelope, decode_server_packet,
    encode_client_envelope, server_envelope,
  },
};

#[derive(Resource)]
pub(crate) struct NetworkClient {
  pub(crate) sender: mpsc::UnboundedSender<ClientEnvelope>,
  pub(crate) sequence: u64,
  pub(crate) connected: bool,
}

#[derive(Resource)]
pub(crate) struct NetworkCommands {
  receiver: mpsc::UnboundedReceiver<ClientEnvelope>,
}

pub(crate) fn start_network_client(mut commands: Commands, mut world: ResMut<ClientWorld>) {
  let (command_sender, command_receiver) = mpsc::unbounded_channel();
  commands.insert_resource(NetworkCommands {
    receiver: command_receiver,
  });
  commands.insert_resource(NetworkClient {
    sender: command_sender,
    sequence: 0,
    connected: false,
  });

  let server_addr = match configured_server_addr() {
    Ok(server_addr) => server_addr,
    Err(err) => {
      world.status = format!("invalid server address: {err}");
      return;
    }
  };
  world.status = format!("connecting to {server_addr}");

  let auth = Authentication::Manual {
    server_addr,
    client_id: configured_client_id(),
    private_key: NETCODE_PRIVATE_KEY,
    protocol_id: NETCODE_PROTOCOL_ID,
  };
  let netcode_client = match NetcodeClient::new(auth, NetcodeConfig::default()) {
    Ok(client) => client,
    Err(err) => {
      world.status = format!("network init failed: {err}");
      return;
    }
  };
  let local_addr = "[::]:0"
    .parse::<SocketAddr>()
    .expect("client wildcard address should parse");
  let entity = commands
    .spawn((UdpIo::default(), LocalAddr(local_addr), netcode_client))
    .id();
  commands.trigger(Connect { entity });
}

pub(crate) fn drain_network_events(
  mut commands: ResMut<NetworkCommands>,
  mut network: ResMut<NetworkClient>,
  mut world: ResMut<ClientWorld>,
  mut client_query: Query<
    (
      &mut MessageSender<ClientPacket>,
      &mut MessageReceiver<ServerPacket>,
      Option<&Connected>,
      Option<&Disconnected>,
    ),
    With<Client>,
  >,
) {
  let Ok((mut sender, mut receiver, connected, disconnected)) = client_query.single_mut() else {
    return;
  };

  for packet in receiver.receive() {
    match decode_server_packet(packet) {
      Ok(message) => handle_server_message(message, &mut world),
      Err(err) => world.status = format!("decode error: {err:#}"),
    }
  }

  if connected.is_some() {
    if !network.connected {
      network.connected = true;
      world.status = "connected".to_string();
      send_hello(&mut sender, &mut world);
    }
  } else if network.connected {
    network.connected = false;
    world.status = disconnected
      .and_then(|state| state.reason.as_ref())
      .map_or_else(
        || "disconnected".to_string(),
        |reason| format!("disconnected: {reason}"),
      );
  }

  if !network.connected {
    return;
  }

  while let Ok(message) = commands.receiver.try_recv() {
    match encode_client_envelope(&message) {
      Ok(packet) => sender.send::<GameChannel>(packet),
      Err(err) => world.status = format!("encode error: {err:#}"),
    }
  }
}

fn send_hello(sender: &mut MessageSender<ClientPacket>, world: &mut ClientWorld) {
  let envelope = ClientEnvelope {
    payload: Some(client_envelope::Payload::Hello(Hello {
      name: configured_client_name(),
      room: std::env::var("GAME_ROOM").unwrap_or_default(),
    })),
  };
  match encode_client_envelope(&envelope) {
    Ok(packet) => sender.send::<GameChannel>(packet),
    Err(err) => world.status = format!("hello encode error: {err:#}"),
  }
}

fn handle_server_message(message: ServerEnvelope, world: &mut ClientWorld) {
  match message.payload {
    Some(server_envelope::Payload::Welcome(welcome)) => {
      world.local_actor_id = Some(welcome.actor_id);
      world.tick = welcome.tick;
      world.status = format!("connected as client {}", welcome.client_id);
    }
    Some(server_envelope::Payload::Snapshot(snapshot)) => {
      world.tick = snapshot.tick;
      if let Some(map) = snapshot.map {
        world.map = Some(map);
      }
      world.actors = snapshot
        .actors
        .into_iter()
        .map(|actor| (actor.id, actor))
        .collect();
    }
    Some(server_envelope::Payload::Pong(pong)) => {
      world.tick = pong.server_tick;
    }
    Some(server_envelope::Payload::Notice(notice)) => {
      world.status = notice.message;
    }
    None => {}
  }
}

fn configured_server_addr() -> Result<SocketAddr, std::net::AddrParseError> {
  std::env::var("GAME_SERVER_ADDR")
    .unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string())
    .parse()
}

fn configured_client_name() -> String {
  std::env::var("GAME_CLIENT_NAME").unwrap_or_else(|_| "bevy-client".to_string())
}

fn configured_client_id() -> u64 {
  if let Ok(client_id) = std::env::var("GAME_CLIENT_ID") {
    if let Ok(client_id) = client_id.parse::<u64>() {
      return client_id;
    }
  }

  let micros = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |duration| duration.as_micros() as u64);
  micros ^ u64::from(std::process::id())
}
