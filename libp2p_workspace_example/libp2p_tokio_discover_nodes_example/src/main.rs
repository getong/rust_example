use libp2p::{
  // floodsub::{self, Floodsub, FloodsubEvent},
  futures::StreamExt,
  identity,
  mdns::{self, Event},
  noise,
  swarm::SwarmEvent,
  tcp,
  yamux,
  PeerId,
  SwarmBuilder,
};
use std::error::Error;
use tokio::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let id_keys = identity::Keypair::generate_ed25519();
  // let peer_id = PeerId::from(id_keys.public()); //1
  // println!("Local peer id: {:?}", peer_id);
  // let transport = libp2p::development_transport(id_keys).await?;

  // let behaviour = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id).unwrap();

  // let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();
  let mut swarm = SwarmBuilder::with_existing_identity(id_keys.clone())
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_dns()?
    .with_behaviour(|key| {
      let peer_id = PeerId::from(key.public());
      // MyBehaviour::new(peer_id).unwrap()
      mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id).unwrap()
      // MyBehaviour {
      //   floodsub: Floodsub::new(peer_id),
      //   mdns,
      // }
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
    .build();

  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
  loop {
    match swarm.select_next_some().await {
      SwarmEvent::NewListenAddr { address, .. } => {
        println!("Listening on local address {:?}", address);
      }
      SwarmEvent::Behaviour(Event::Discovered(peers)) => {
        for (peer, addr) in peers {
          println!("discovered {} {}", peer, addr);
        }
      }
      SwarmEvent::Behaviour(Event::Expired(expired)) => {
        for (peer, addr) in expired {
          println!("expired {} {}", peer, addr);
        }
      }
      _ => {}
    }
  }
}
