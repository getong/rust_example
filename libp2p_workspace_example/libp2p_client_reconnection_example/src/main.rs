use libp2p::{
  Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
  futures::StreamExt,
  identity::Keypair,
  kad::{
    Behaviour as KadBehavior, Config as KadConfig, Event as KadEvent,
    store::MemoryStore as KadInMemory,
  },
  noise::Config as NoiseConfig,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp::Config as TcpConfig,
  yamux::Config as YamuxConfig,
};
use tokio::time::{Duration, sleep};
use tracing::info;

#[derive(NetworkBehaviour)]
struct MyBehaviour {
  kademlia: KadBehavior<KadInMemory>,
}

const BOOTSTRAP_PEER_NAME: &str = "1AidRwssZM3a86w66WJhtc9rYS3TmWZ6XQHqNdMnz1UPUR";
const BOOTSTRAP_ADDR: &str = "/ip4/127.0.0.1/tcp/8080";
const STREAM_PROTOCOL_NAME: &str = "/agent/connection/1.0.0";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let local_key = Keypair::generate_secp256k1();
  let local_peer_id = PeerId::from(local_key.public());

  let bootstrap_addr: Multiaddr = BOOTSTRAP_ADDR
    .parse()
    .map_err(|_| "Failed to parse bootstrap address")?;

  let bootstrap_peer_id = PeerId::from_bytes(
    &bs58::decode(BOOTSTRAP_PEER_NAME)
      .into_vec()
      .map_err(|e| format!("Invalid base58 string: {}", e))?,
  )
  .map_err(|e| format!("Failed to parse PeerId from base58: {}", e))?;

  info!("Local Peer ID: {}", local_peer_id);

  let mut backoff = Duration::from_secs(1);
  loop {
    match monitor_connection(
      local_key.clone(),
      local_peer_id,
      bootstrap_peer_id,
      bootstrap_addr.clone(),
    )
    .await
    {
      Ok(_) => {
        info!("Monitor task exited cleanly.");
        backoff = Duration::from_secs(1); // Reset backoff on success
      }
      Err(e) => {
        info!("Monitor task failed: {}. Restarting...", e);
        sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(60)); // Cap backoff
      }
    }
  }
}

async fn monitor_connection(
  local_key: Keypair,
  local_peer_id: PeerId,
  bootstrap_peer_id: PeerId,
  bootstrap_addr: Multiaddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let mut swarm = configure_swarm(local_key, local_peer_id, bootstrap_peer_id, bootstrap_addr)?;

  // Main monitoring loop
  loop {
    tokio::select! {
        event = swarm.select_next_some() => {
            if let Err(e) = handle_event(event, bootstrap_peer_id).await {
                info!("Error handling swarm event: {}", e);
                return Err(e);
            }
        },
        _ = sleep(Duration::from_secs(1)) => {
            // Handle periodic actions here if needed
        }
    }
  }
}

fn configure_swarm(
  local_key: Keypair,
  local_peer_id: PeerId,
  bootstrap_peer_id: PeerId,
  bootstrap_addr: Multiaddr,
) -> Result<Swarm<MyBehaviour>, Box<dyn std::error::Error + Send + Sync>> {
  let mut swarm = SwarmBuilder::with_existing_identity(local_key)
    .with_tokio()
    .with_tcp(TcpConfig::default(), NoiseConfig::new, YamuxConfig::default)?
    .with_behaviour(|_key| {
      let kad_config = KadConfig::new(StreamProtocol::new(STREAM_PROTOCOL_NAME));
      let kad_memory = KadInMemory::new(local_peer_id);
      let mut kad = KadBehavior::with_config(local_peer_id, kad_memory, kad_config);

      // Add a bootstrap node
      kad.add_address(&bootstrap_peer_id, bootstrap_addr.clone());

      MyBehaviour { kademlia: kad }
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(10)))
    .build();

  swarm
    .dial(bootstrap_addr)
    .map_err(|_| "libp2p connect to boot node fail, might be network issue")?;

  // Listen on a local address
  let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
  swarm.listen_on(listen_addr.clone())?;

  info!("Listening on {}", listen_addr.clone());
  Ok(swarm)
}

async fn handle_event(
  event: SwarmEvent<MyBehaviourEvent>,
  bootstrap_peer_id: PeerId,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  match event {
    SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(KadEvent::RoutingUpdated {
      peer,
      is_new_peer,
      ..
    })) => {
      if peer == bootstrap_peer_id {
        info!(
          "Connected to bootstrap peer: {} (is_new_peer: {})",
          peer, is_new_peer
        );
      }
    }
    SwarmEvent::ConnectionClosed { peer_id, .. } => {
      if peer_id == bootstrap_peer_id {
        info!("Connection to bootstrap peer {} lost", peer_id);
        return Err(format!("Lost connection to bootstrap peer: {}", peer_id).into());
      }
    }
    SwarmEvent::NewListenAddr { address, .. } => {
      info!("Swarm is now listening on address: {}", address);
    }
    SwarmEvent::IncomingConnection { .. } => {
      info!("Incoming connection detected");
    }
    _ => {
      // Ignore other events for now
    }
  }
  Ok(())
}
