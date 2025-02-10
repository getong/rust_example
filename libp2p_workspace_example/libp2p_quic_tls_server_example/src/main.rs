use anyhow::Result;
use futures::StreamExt;
use libp2p::{
  PeerId, SwarmBuilder, identity,
  kad::{self, Event as KademliaEvent, store::MemoryStore},
  noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tls, yamux,
};
use tokio::time::Duration;

// copy from https://github.com/libp2p/rust-libp2p/blob/master/libp2p/src/builder.rs

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "MyBehaviourEvent")]
struct MyBehaviour {
  kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

impl MyBehaviour {
  fn new(peer_id: PeerId) -> Result<Self> {
    let kademlia = kad::Behaviour::new(peer_id, MemoryStore::new(peer_id));

    Ok(Self { kademlia })
  }
}

#[derive(Debug)]
pub enum MyBehaviourEvent {
  Kademlia(KademliaEvent),
}

impl From<KademliaEvent> for MyBehaviourEvent {
  fn from(event: KademliaEvent) -> Self {
    MyBehaviourEvent::Kademlia(event)
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  let key_pair = identity::Keypair::generate_ed25519();

  let mut swarm = SwarmBuilder::with_existing_identity(key_pair)
    .with_tokio()
    .with_tcp(
      Default::default(),
      (tls::Config::new, noise::Config::new),
      yamux::Config::default,
    )?
    .with_quic()
    .with_dns()?
    .with_behaviour(|key| {
      let peer_id = PeerId::from(key.public());
      MyBehaviour::new(peer_id).unwrap()
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
    .build();

  swarm.listen_on("/ip4/0.0.0.0/tcp/3000".parse()?)?;
  swarm.listen_on("/ip4/0.0.0.0/udp/3001/quic-v1".parse()?)?;

  loop {
    tokio::select! {
        event = swarm.select_next_some() => {
            if let SwarmEvent::NewListenAddr { address, .. } = event {
                println!("Listening on: {address}");
            }
        }
    }
  }
}
