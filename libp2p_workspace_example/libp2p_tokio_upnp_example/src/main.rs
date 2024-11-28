use std::error::Error;

use futures::prelude::*;
use libp2p::{noise, swarm::SwarmEvent, upnp, yamux, Multiaddr};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let mut swarm = libp2p::SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_tcp(
      Default::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_behaviour(|_| upnp::tokio::Behaviour::default())?
    .build();

  // Tell the swarm to listen on all interfaces and a random, OS-assigned
  // port.
  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

  // Dial the peer identified by the multi-address given as the second
  // command-line argument, if any.
  if let Some(addr) = std::env::args().nth(1) {
    let remote: Multiaddr = addr.parse()?;
    swarm.dial(remote)?;
    println!("Dialed {addr}")
  }

  loop {
    match swarm.select_next_some().await {
      SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {address:?}"),
      SwarmEvent::Behaviour(upnp::Event::NewExternalAddr(addr)) => {
        println!("New external address: {addr}");
      }
      SwarmEvent::Behaviour(upnp::Event::GatewayNotFound) => {
        println!("Gateway does not support UPnP");
        break;
      }
      SwarmEvent::Behaviour(upnp::Event::NonRoutableGateway) => {
        println!(
          "Gateway is not exposed directly to the public Internet, i.e. it itself has a private \
           IP address."
        );
        break;
      }
      _ => {}
    }
  }
  Ok(())
}
