use std::{
  collections::HashMap,
  net::SocketAddr,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
  time::Duration,
};

use anyhow::{Context, Result, bail};
use quinn::crypto::rustls::QuicServerConfig;
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::{
  runtime::Runtime,
  sync::{mpsc, oneshot},
  time::timeout,
};

use crate::{
  protocol::{
    ALPN_PROTOCOL, ClientEnvelope, ServerEnvelope, client_envelope, read_client_envelope,
    write_server_envelope,
  },
  routing::{CLIENT_OUTBOUND_BUFFER, GatewayEvent, ShardCommand, ShardHandle},
};

struct ClientConnection {
  shard_id: usize,
  sender: mpsc::Sender<ServerEnvelope>,
}

const CLIENT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) fn run_gateway(
  bind_addr: &str,
  shards: Vec<ShardHandle>,
  gateway_sender: mpsc::Sender<GatewayEvent>,
  gateway_receiver: mpsc::Receiver<GatewayEvent>,
) -> Result<()> {
  let runtime = Runtime::new().context("tokio runtime should start")?;
  runtime.block_on(run_quic_gateway(
    bind_addr,
    shards,
    gateway_sender,
    gateway_receiver,
  ))
}

async fn run_quic_gateway(
  bind_addr: &str,
  shards: Vec<ShardHandle>,
  gateway_sender: mpsc::Sender<GatewayEvent>,
  mut gateway_receiver: mpsc::Receiver<GatewayEvent>,
) -> Result<()> {
  if shards.is_empty() {
    bail!("gateway requires at least one shard");
  }

  let endpoint = quinn::Endpoint::server(server_config()?, bind_addr.parse::<SocketAddr>()?)
    .context("failed to bind quinn gateway endpoint")?;
  let shards = Arc::new(shards);
  let next_client_id = Arc::new(AtomicU64::new(1));
  let mut clients = HashMap::new();

  println!(
    "game_server gateway listening on {bind_addr}, shards={}",
    shards.len()
  );

  loop {
    tokio::select! {
      incoming = endpoint.accept() => {
        let Some(incoming) = incoming else {
          break;
        };

        let client_id = next_client_id.fetch_add(1, Ordering::Relaxed);
        let connection_gateway_sender = gateway_sender.clone();
        let connection_shards = Arc::clone(&shards);
        tokio::spawn(async move {
          if let Err(err) =
            handle_connection(client_id, incoming, connection_gateway_sender, connection_shards).await
          {
            eprintln!("client {client_id} connection error: {err:#}");
          }
        });
      }
      event = gateway_receiver.recv() => {
        let Some(event) = event else {
          break;
        };
        handle_gateway_event(event, &mut clients);
      }
    }
  }

  Ok(())
}

fn handle_gateway_event(event: GatewayEvent, clients: &mut HashMap<u64, ClientConnection>) {
  match event {
    GatewayEvent::ClientReady {
      client_id,
      shard_id,
      sender,
      ack,
    } => {
      clients.insert(client_id, ClientConnection { shard_id, sender });
      let _ = ack.send(());
    }
    GatewayEvent::Send { client_id, message } => {
      let should_remove =
        clients
          .get(&client_id)
          .is_some_and(|client| match client.sender.try_send(message) {
            Ok(()) => false,
            Err(mpsc::error::TrySendError::Full(_)) => false,
            Err(mpsc::error::TrySendError::Closed(_)) => true,
          });

      if should_remove {
        clients.remove(&client_id);
      }
    }
    GatewayEvent::ClientDisconnected {
      client_id,
      shard_id,
    } => {
      if clients
        .get(&client_id)
        .is_some_and(|client| client.shard_id == shard_id)
      {
        clients.remove(&client_id);
      }
    }
  }
}

async fn handle_connection(
  client_id: u64,
  incoming: quinn::Incoming,
  gateway_sender: mpsc::Sender<GatewayEvent>,
  shards: Arc<Vec<ShardHandle>>,
) -> Result<()> {
  let connection = incoming
    .await
    .context("failed to accept quinn connection")?;
  let (send, mut recv) = timeout(CLIENT_HANDSHAKE_TIMEOUT, connection.accept_bi())
    .await
    .context("client timed out before opening stream")?
    .context("failed to accept client stream")?;
  let first_message = timeout(CLIENT_HANDSHAKE_TIMEOUT, read_client_envelope(&mut recv))
    .await
    .context("client timed out before sending hello")?
    .context("failed to read client hello")?
    .context("client disconnected before sending hello")?;
  let shard = choose_shard(client_id, &first_message, &shards)?.clone();
  let (outbound_sender, outbound_receiver) = mpsc::channel(CLIENT_OUTBOUND_BUFFER);

  tokio::spawn(write_loop(
    client_id,
    shard.id,
    send,
    outbound_receiver,
    gateway_sender.clone(),
    shard.sender.clone(),
  ));

  register_client(&gateway_sender, client_id, shard.id, outbound_sender).await?;
  if let Err(err) = send_shard_command(&shard.sender, ShardCommand::Connected { client_id }).await {
    disconnect_client(client_id, shard.id, &gateway_sender, &shard.sender).await;
    return Err(err);
  }
  if let Err(err) = send_shard_command(
    &shard.sender,
    ShardCommand::Message {
      client_id,
      message: first_message,
    },
  )
  .await
  {
    disconnect_client(client_id, shard.id, &gateway_sender, &shard.sender).await;
    return Err(err);
  }

  let result = read_client_messages(client_id, &mut recv, &shard.sender).await;
  disconnect_client(client_id, shard.id, &gateway_sender, &shard.sender).await;
  result
}

async fn read_client_messages(
  client_id: u64,
  recv: &mut quinn::RecvStream,
  shard_sender: &mpsc::Sender<ShardCommand>,
) -> Result<()> {
  while let Some(message) = read_client_envelope(recv).await? {
    match shard_sender.try_send(ShardCommand::Message { client_id, message }) {
      Ok(()) => {}
      Err(mpsc::error::TrySendError::Full(_)) => {}
      Err(mpsc::error::TrySendError::Closed(_)) => bail!("client shard input channel closed"),
    }
  }

  Ok(())
}

async fn register_client(
  gateway_sender: &mpsc::Sender<GatewayEvent>,
  client_id: u64,
  shard_id: usize,
  sender: mpsc::Sender<ServerEnvelope>,
) -> Result<()> {
  let (ack, registered) = oneshot::channel();
  gateway_sender
    .send(GatewayEvent::ClientReady {
      client_id,
      shard_id,
      sender,
      ack,
    })
    .await
    .context("failed to register client with gateway")?;
  registered
    .await
    .context("gateway stopped before client registration completed")
}

async fn send_shard_command(
  shard_sender: &mpsc::Sender<ShardCommand>,
  command: ShardCommand,
) -> Result<()> {
  shard_sender
    .send(command)
    .await
    .context("failed to send command to shard")
}

async fn disconnect_client(
  client_id: u64,
  shard_id: usize,
  gateway_sender: &mpsc::Sender<GatewayEvent>,
  shard_sender: &mpsc::Sender<ShardCommand>,
) {
  let _ = shard_sender.try_send(ShardCommand::Disconnected { client_id });
  let _ = gateway_sender
    .send(GatewayEvent::ClientDisconnected {
      client_id,
      shard_id,
    })
    .await;
}

async fn write_loop(
  client_id: u64,
  shard_id: usize,
  mut send: quinn::SendStream,
  mut receiver: mpsc::Receiver<ServerEnvelope>,
  gateway_sender: mpsc::Sender<GatewayEvent>,
  shard_sender: mpsc::Sender<ShardCommand>,
) {
  while let Some(message) = receiver.recv().await {
    if let Err(err) = write_server_envelope(&mut send, &message).await {
      eprintln!("client {client_id} write error: {err:#}");
      disconnect_client(client_id, shard_id, &gateway_sender, &shard_sender).await;
      return;
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
