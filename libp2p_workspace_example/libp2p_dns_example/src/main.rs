use anyhow::Result;
use futures::StreamExt;
use libp2p::{
  PeerId, SwarmBuilder, Transport,
  core::transport::upgrade::Version,
  dns, identity,
  kad::{self, Event as KademliaEvent, store::MemoryStore},
  noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux,
};
use tokio::time::Duration;

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
    .with_other_transport(|key| {
      let noise_config = noise::Config::new(key).unwrap();
      let mut yamux_config = yamux::Config::default();
      yamux_config.set_max_num_streams(1024 * 1024);
      let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
      let base_transport = dns::tokio::Transport::system(base_transport)
        .expect("DNS")
        .boxed();

      base_transport
        .upgrade(Version::V1Lazy)
        .authenticate(noise_config)
        .multiplex(yamux_config)
    })?
    .with_dns()?
    .with_behaviour(|key| {
      let peer_id = PeerId::from(key.public());
      MyBehaviour::new(peer_id).unwrap()
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
    .build();

  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
  // swarm.listen_on("/dns4/localhost/tcp/0".parse()?)?;

  loop {
    tokio::select! {
        event = swarm.select_next_some() => {
            if let SwarmEvent::NewListenAddr { address, .. } = event {
                // Print the listen address and show the equivalent DNS address
                println!("Listening on: {address}");
                if let Some(port) = address.iter().last() {
                    println!("You can dial this node using: /dns4/localhost/tcp/{port}");
                }
            }
        }
    }
  }
}
