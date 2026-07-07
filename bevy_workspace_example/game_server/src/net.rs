use std::{
  collections::HashMap,
  net::SocketAddr,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use anyhow::{Context, Result};
use bevy::prelude::*;
use quinn::crypto::rustls::QuicServerConfig;
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::{runtime::Runtime, sync::mpsc};

use crate::{
  game::{
    ActorId, ActorType, ArenaPosition, NextActorId, PlayerInputState, ServerTick, SnapshotClock,
    Vitals, actor_state, spawn_player,
  },
  protocol::{
    ALPN_PROTOCOL, ClientEnvelope, DEFAULT_SERVER_ADDR, ServerEnvelope, Welcome, WorldSnapshot,
    client_envelope, read_client_envelope, server_envelope, write_server_envelope,
  },
  terrain::{LevelMap, map_state},
};

#[allow(dead_code)]
#[derive(Resource)]
pub(crate) struct NetworkRuntime(Runtime);

#[derive(Resource)]
pub(crate) struct NetworkEvents {
  receiver: mpsc::UnboundedReceiver<NetworkEvent>,
}

#[derive(Resource, Default)]
pub(crate) struct Clients {
  clients: HashMap<u64, ClientState>,
}

struct ClientState {
  sender: mpsc::UnboundedSender<ServerEnvelope>,
  player: Option<Entity>,
  actor_id: Option<ActorId>,
  name: String,
}

enum NetworkEvent {
  Connected {
    client_id: u64,
    sender: mpsc::UnboundedSender<ServerEnvelope>,
  },
  Message {
    client_id: u64,
    message: ClientEnvelope,
  },
  Disconnected {
    client_id: u64,
  },
}

pub(crate) fn start_network_server(mut commands: Commands) {
  let (event_sender, event_receiver) = mpsc::unbounded_channel();
  let runtime = Runtime::new().expect("tokio runtime should start");
  runtime.spawn(async move {
    if let Err(err) = run_quic_server(DEFAULT_SERVER_ADDR, event_sender).await {
      eprintln!("game_server network error: {err:#}");
    }
  });

  commands.insert_resource(NetworkRuntime(runtime));
  commands.insert_resource(NetworkEvents {
    receiver: event_receiver,
  });
  info!("game_server listening on {DEFAULT_SERVER_ADDR}");
}

pub(crate) fn drain_network_events(
  mut commands: Commands,
  mut events: ResMut<NetworkEvents>,
  mut clients: ResMut<Clients>,
  mut actor_ids: ResMut<NextActorId>,
  level_map: Res<LevelMap>,
  tick: Res<ServerTick>,
  mut player_inputs: Query<&mut PlayerInputState>,
) {
  while let Ok(event) = events.receiver.try_recv() {
    match event {
      NetworkEvent::Connected { client_id, sender } => {
        info!("client {client_id} connected");
        clients.clients.insert(
          client_id,
          ClientState {
            sender,
            player: None,
            actor_id: None,
            name: format!("Player {client_id}"),
          },
        );
      }
      NetworkEvent::Message { client_id, message } => {
        handle_client_message(
          &mut commands,
          &mut clients,
          &mut actor_ids,
          &level_map,
          &mut player_inputs,
          tick.0,
          client_id,
          message,
        );
      }
      NetworkEvent::Disconnected { client_id } => {
        if let Some(client) = clients.clients.remove(&client_id) {
          if let Some(player) = client.player {
            commands.entity(player).despawn();
          }
          info!("client {client_id} disconnected");
        }
      }
    }
  }
}

pub(crate) fn broadcast_snapshots(
  time: Res<Time>,
  tick: Res<ServerTick>,
  mut clock: ResMut<SnapshotClock>,
  level_map: Res<LevelMap>,
  clients: Res<Clients>,
  actors: Query<(&ActorId, &ActorType, &ArenaPosition, &Vitals)>,
) {
  clock.0.tick(time.delta());
  if !clock.0.just_finished() {
    return;
  }

  let snapshot = WorldSnapshot {
    tick: tick.0,
    actors: actors
      .iter()
      .map(|(id, kind, position, vitals)| actor_state(*id, *kind, *position, *vitals))
      .collect(),
    map: Some(map_state(&level_map)),
  };
  let envelope = ServerEnvelope {
    payload: Some(server_envelope::Payload::Snapshot(snapshot)),
  };

  for client in clients.clients.values() {
    let _ = client.sender.send(envelope.clone());
  }
}

fn handle_client_message(
  commands: &mut Commands,
  clients: &mut Clients,
  actor_ids: &mut NextActorId,
  level_map: &LevelMap,
  player_inputs: &mut Query<&mut PlayerInputState>,
  tick: u64,
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
        let _ = client.sender.send(welcome);
      }
    }
    Some(client_envelope::Payload::Input(input)) => {
      let Some(player) = client.player else {
        return;
      };
      let Ok(mut player_input) = player_inputs.get_mut(player) else {
        return;
      };

      player_input.direction = Vec2::new(input.x, input.y).clamp_length_max(1.0);
    }
    Some(client_envelope::Payload::Ping(ping)) => {
      let pong = ServerEnvelope {
        payload: Some(server_envelope::Payload::Pong(crate::protocol::Pong {
          client_time_ms: ping.client_time_ms,
          server_tick: tick,
        })),
      };
      let _ = client.sender.send(pong);
    }
    None => {}
  }
}

async fn run_quic_server(
  bind_addr: &str,
  event_sender: mpsc::UnboundedSender<NetworkEvent>,
) -> Result<()> {
  let endpoint = quinn::Endpoint::server(server_config()?, bind_addr.parse::<SocketAddr>()?)
    .context("failed to bind quinn server endpoint")?;
  let next_client_id = Arc::new(AtomicU64::new(1));

  while let Some(incoming) = endpoint.accept().await {
    let client_id = next_client_id.fetch_add(1, Ordering::Relaxed);
    let events = event_sender.clone();
    tokio::spawn(async move {
      if let Err(err) = handle_connection(client_id, incoming, events.clone()).await {
        eprintln!("client {client_id} connection error: {err:#}");
        let _ = events.send(NetworkEvent::Disconnected { client_id });
      }
    });
  }

  Ok(())
}

async fn handle_connection(
  client_id: u64,
  incoming: quinn::Incoming,
  event_sender: mpsc::UnboundedSender<NetworkEvent>,
) -> Result<()> {
  let connection = incoming
    .await
    .context("failed to accept quinn connection")?;
  let (send, mut recv) = connection
    .accept_bi()
    .await
    .context("failed to accept client stream")?;
  let (outbound_sender, outbound_receiver) = mpsc::unbounded_channel();

  event_sender
    .send(NetworkEvent::Connected {
      client_id,
      sender: outbound_sender,
    })
    .context("failed to publish client connection")?;

  tokio::spawn(write_loop(client_id, send, outbound_receiver));

  while let Some(message) = read_client_envelope(&mut recv).await? {
    event_sender
      .send(NetworkEvent::Message { client_id, message })
      .context("failed to publish client message")?;
  }

  let _ = event_sender.send(NetworkEvent::Disconnected { client_id });
  Ok(())
}

async fn write_loop(
  client_id: u64,
  mut send: quinn::SendStream,
  mut receiver: mpsc::UnboundedReceiver<ServerEnvelope>,
) {
  while let Some(message) = receiver.recv().await {
    if let Err(err) = write_server_envelope(&mut send, &message).await {
      eprintln!("client {client_id} write error: {err:#}");
      return;
    }
  }
}

fn server_config() -> Result<quinn::ServerConfig> {
  let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])
    .context("failed to generate self-signed certificate")?;
  let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()));
  let cert_chain = vec![cert.cert.der().clone()];

  let mut server_crypto = rustls::ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(cert_chain, key)
    .context("failed to build rustls server config")?;
  server_crypto.alpn_protocols = vec![ALPN_PROTOCOL.to_vec()];

  Ok(quinn::ServerConfig::with_crypto(Arc::new(
    QuicServerConfig::try_from(server_crypto).context("failed to build quic server config")?,
  )))
}
