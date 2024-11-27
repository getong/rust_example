use futures::{executor::block_on, future, StreamExt};
use libp2p::{
  identity, noise,
  ping::{Behaviour, Config},
  swarm::SwarmEvent,
  tcp, yamux, Multiaddr, PeerId, SwarmBuilder,
};
use std::{error::Error, task::Poll, time::Duration};

fn main() -> Result<(), Box<dyn Error>> {
  let local_key = identity::Keypair::generate_ed25519();
  let local_peer_id = PeerId::from(local_key.public());
  println!("Local peer id: {:?}", local_peer_id);

  // let mut swarm = SwarmBuilder::without_executor(transport, behaviour, local_peer_id).build();
  let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_dns()?
    .with_behaviour(|_key| Behaviour::new(Config::default()))?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
    .build();

  // Tell the swarm to listen on all interfaces and a random, OS-assigned
  // port.
  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

  // Dial the peer identified by the multi-address given as the second
  // command-line argument, if any.
  if let Some(addr) = std::env::args().nth(1) {
    let remote: Multiaddr = addr.parse()?;
    swarm.dial(remote)?;
    println!("Dialed {}", addr)
  }

  block_on(future::poll_fn(move |cx| loop {
    match swarm.poll_next_unpin(cx) {
      Poll::Ready(Some(event)) => match event {
        SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {:?}", address),
        SwarmEvent::Behaviour(event) => println!("{:?}", event),
        _ => {}
      },
      Poll::Ready(None) => return Poll::Ready(()),
      Poll::Pending => return Poll::Pending,
    }
  }));

  Ok(())
}
