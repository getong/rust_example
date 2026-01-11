use libp2p::{
  Multiaddr, StreamProtocol,
  kad::store::MemoryStore,
  swarm::{NetworkBehaviour, Swarm, SwarmEvent},
};
use libp2p_iroh::{Transport, TransportTrait};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
  kademlia: libp2p::kad::Behaviour<MemoryStore>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let keypair = libp2p::identity::Keypair::generate_ed25519();
  let peer_id = keypair.public().to_peer_id();

  let transport = Transport::new(Some(&keypair)).await?.boxed();

  println!(
    "Copy and paste this in a second terminal, press enter to connect back to this node from \
     anywhere:"
  );
  println!("  /p2p/{peer_id}");

  let kad_config = libp2p::kad::Config::new(StreamProtocol::new("/example/kad/1.0.0"));
  let store = MemoryStore::new(peer_id);
  let behaviour = MyBehaviour {
    kademlia: libp2p::kad::Behaviour::with_config(peer_id, store, kad_config),
  };

  let mut swarm = Swarm::new(
    transport,
    behaviour,
    peer_id,
    libp2p::swarm::Config::with_executor(Box::new(|fut| {
      tokio::spawn(fut);
    }))
    .with_idle_connection_timeout(std::time::Duration::from_secs(300)),
  );

  // Our listener address looks like this: /p2p/12D3KooWEUowGZ...
  swarm.listen_on(Multiaddr::empty())?;

  // Mini cli to dial other peers
  let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
  tokio::spawn(async move {
    print!("> ");
    let mut stdin = std::io::stdin().lock();
    let mut line = String::new();
    if std::io::BufRead::read_line(&mut stdin, &mut line).is_ok() && !line.is_empty() {
      if let Ok(peer_multiaddr) = line.trim().parse::<Multiaddr>() {
        tx.send(peer_multiaddr).unwrap();
      };
    }
  });

  loop {
    tokio::select! {
      event = futures::StreamExt::select_next_some(&mut swarm) => {
        if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
          println!("Connection established with {peer_id}!");
        }
      }
      Some(addr) = rx.recv() => {
        println!("Dialing {addr}...");
        swarm.dial(addr)?;
      }
    }
  }
}
