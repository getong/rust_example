use libp2p::{
  Multiaddr, PeerId, StreamProtocol, SwarmBuilder,
  futures::StreamExt,
  identity::Keypair,
  kad::{
    Behaviour as KadBehavior, Config as KadConfig, Event as KadEvent,
    store::MemoryStore as KadInMemory,
  },
  noise::Config as NoiceConfig,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp::Config as TcpConfig,
  yamux::Config as YamuxConfig,
};
use tokio::{
  spawn,
  time::{Duration, sleep},
};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
  kademlia: KadBehavior<KadInMemory>,
}

async fn monitor_connection(
  local_peer_id: PeerId,
  bootstrap_peer_id: PeerId,
  bootstrap_addr: Multiaddr,
  local_key: Keypair,
) -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
  let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
    .with_tokio()
    .with_tcp(TcpConfig::default(), NoiceConfig::new, YamuxConfig::default)?
    .with_behaviour(|_key| {
      let kad_config = KadConfig::new(StreamProtocol::new("/agent/connection/1.0.0"));

      let kad_memory = KadInMemory::new(local_peer_id);
      let mut kad = KadBehavior::with_config(local_peer_id, kad_memory, kad_config);

      // Add a bootstrap node
      kad.add_address(&bootstrap_peer_id, bootstrap_addr.clone());

      MyBehaviour { kademlia: kad }
    })?
    .build();

  // Listen on a local address
  let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
  swarm.listen_on(listen_addr.clone())?;

  println!("Listening on {}", listen_addr.clone());

  // Main monitoring loop
  loop {
    match swarm.select_next_some().await {
      SwarmEvent::Behaviour(MyBehaviourEvent::Kademlia(KadEvent::RoutingUpdated {
        peer,
        is_new_peer,
        ..
      })) => {
        if peer == bootstrap_peer_id {
          println!(
            "Connected to bootstrap peer: {} (is_new_peer: {})",
            peer, is_new_peer
          );
        }
      }
      SwarmEvent::ConnectionClosed { peer_id, .. } => {
        if peer_id == bootstrap_peer_id {
          println!("Connection to bootstrap peer {} lost", peer_id);
          return Ok(()); // Exit the task if connection is lost
        }
      }
      _ => {}
    }

    // Delay to prevent busy-waiting
    sleep(Duration::from_millis(100)).await;
  }
}

#[tokio::main]
async fn main() {
  let local_key = Keypair::generate_secp256k1();
  let local_peer_id = PeerId::from(local_key.public());
  // fake name here, donot use in production
  let remote_peer_name = "1AidRwssZM3a86w66WJhtc9rYS3TmWZ6XQHqNdMnz1UPUR";
  let bootstrap_peer_id = PeerId::from_bytes(
    &bs58::decode(remote_peer_name)
      .into_vec()
      .expect("Invalid base58 string"),
  )
  .expect("Failed to parse PeerId from base58");
  let bootstrap_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8080".parse().unwrap();

  println!("Local Peer ID: {}", local_peer_id);

  let mut task_handle = spawn(monitor_connection(
    local_peer_id,
    bootstrap_peer_id,
    bootstrap_addr.clone(),
    local_key.clone(),
  ));

  loop {
    if task_handle.is_finished() {
      println!("Monitor task has exited, restarting...");
      task_handle = spawn(monitor_connection(
        local_peer_id,
        bootstrap_peer_id,
        bootstrap_addr.clone(),
        local_key.clone(),
      ));
    }

    sleep(Duration::from_secs(5)).await; // Check every 5 seconds
  }
}
