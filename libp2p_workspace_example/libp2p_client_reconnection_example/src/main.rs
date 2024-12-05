use libp2p::{
  Multiaddr, PeerId, StreamProtocol, SwarmBuilder,
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

async fn monitor_connection(
  local_key: Keypair,
  local_peer_id: PeerId,

  bootstrap_peer_id: PeerId,
  bootstrap_addr: Multiaddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
    .with_tokio()
    .with_tcp(TcpConfig::default(), NoiseConfig::new, YamuxConfig::default)?
    .with_behaviour(|_key| {
      let kad_config = KadConfig::new(StreamProtocol::new("/agent/connection/1.0.0"));

      let kad_memory = KadInMemory::new(local_peer_id);
      let mut kad = KadBehavior::with_config(local_peer_id, kad_memory, kad_config);

      // Add a bootstrap node
      kad.add_address(&bootstrap_peer_id, bootstrap_addr.clone());

      MyBehaviour { kademlia: kad }
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(10)))
    .build();

  // Listen on a local address
  let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
  swarm.listen_on(listen_addr.clone())?;

  println!("Listening on {}", listen_addr.clone());

  // Main monitoring loop
  loop {
    tokio::select! {
        event = swarm.select_next_some() => match event {
            SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(KadEvent::RoutingUpdated { peer, is_new_peer, .. })) => {
                if peer == bootstrap_peer_id {
                    println!("Connected to bootstrap peer: {} (is_new_peer: {})", peer, is_new_peer);
                }
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                if peer_id == bootstrap_peer_id {
                    println!("Connection to bootstrap peer {} lost", peer_id);
                    return Ok(()); // Exit the task if connection is lost
                }
            }
            _ => {}
        },
        _ = sleep(Duration::from_secs(1)) => {
            // Handle periodic actions here if needed
        }
    }
  }
}

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

  loop {
    match monitor_connection(
      local_key.clone(),
      local_peer_id,
      bootstrap_peer_id,
      bootstrap_addr.clone(),
    )
    .await
    {
      Ok(_) => info!("Monitor task exited cleanly."),
      Err(e) => info!("Monitor task failed: {}. Restarting...", e),
    }
    sleep(Duration::from_secs(5)).await;
  }
}
