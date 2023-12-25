use libp2p::{
  // core::transport::upgrade::Version,
  futures::StreamExt,
  identity::Keypair,
  mdns,
  noise,
  swarm::SwarmEvent,
  tcp,
  yamux,
  Multiaddr,
  PeerId,
  SwarmBuilder,
  // Transport,
};
use std::error::Error;
use tokio::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Generate a new keypair for our local peer
  let local_keypair = Keypair::generate_secp256k1();

  // Create a TCP transport
  // let transport = tcp::tokio::Transport::default()
  //   .upgrade(Version::V1Lazy)
  //   .authenticate(noise::Config::new(&local_keypair).unwrap())
  //   .multiplex(yamux::Config::default())
  //   .boxed();

  // // Create an identity for our local peer
  // let local_peer_id = PeerId::from_public_key(&local_keypair.public());

  // // Create an mDNS service
  // let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id).unwrap();

  // // Create a Swarm with the transport, mdns, and our local peer identity
  // let mut swarm = SwarmBuilder::with_tokio_executor(transport, mdns, local_peer_id).build();

  let mut swarm = SwarmBuilder::with_existing_identity(local_keypair.clone())
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
  // Start listening on a random TCP port
  let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/0".parse()?;
  swarm.listen_on(listen_addr)?;

  // println!("Local peer id: {:?}", local_peer_id);

  // Process events in the swarm
  loop {
    // match swarm.select_next_some().await {
    //     SwarmEvent::Behaviour(mdns::Event::Discovered(peers)) => {
    //         for (peer_id, _addr) in peers {
    //             println!("Discovered peer: {:?}", peer_id);
    //         }
    //     }
    //     _ => {}
    // }
    if let SwarmEvent::Behaviour(mdns::Event::Discovered(peers)) = swarm.select_next_some().await {
      for (peer_id, _addr) in peers {
        println!("Discovered peer: {:?}", peer_id);
      }
    }
  }
}
