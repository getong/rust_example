use std::{
  error::Error,
  net::{Ipv4Addr, Ipv6Addr},
};

use clap::Parser;
use futures::StreamExt;
use libp2p::{
  core::{multiaddr::Protocol, Multiaddr},
  identify, identity, noise, ping, relay,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux, PeerId,
};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let opt = Opt::parse();

  // Create a static known PeerId based on given secret
  let local_key: identity::Keypair = generate_ed25519(opt.secret_key_seed);
  let peer_id = PeerId::from(local_key.public());

  println!("Generated PeerId: {:?}", peer_id);

  let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_quic()
    .with_behaviour(|key| Behaviour {
      relay: relay::Behaviour::new(key.public().to_peer_id(), Default::default()),
      ping: ping::Behaviour::new(ping::Config::new()),
      identify: identify::Behaviour::new(identify::Config::new(
        "/TODO/0.0.1".to_string(),
        key.public(),
      )),
    })?
    .build();

  // Listen on all interfaces
  let listen_addr_tcp = Multiaddr::empty()
    .with(match opt.use_ipv6 {
      Some(true) => Protocol::from(Ipv6Addr::UNSPECIFIED),
      _ => Protocol::from(Ipv4Addr::UNSPECIFIED),
    })
    .with(Protocol::Tcp(opt.port));
  swarm.listen_on(listen_addr_tcp)?;

  let listen_addr_quic = Multiaddr::empty()
    .with(match opt.use_ipv6 {
      Some(true) => Protocol::from(Ipv6Addr::UNSPECIFIED),
      _ => Protocol::from(Ipv4Addr::UNSPECIFIED),
    })
    .with(Protocol::Udp(opt.port))
    .with(Protocol::QuicV1);
  swarm.listen_on(listen_addr_quic)?;

  loop {
    match swarm.next().await.expect("Infinite Stream.") {
      SwarmEvent::Behaviour(event) => {
        if let BehaviourEvent::Identify(identify::Event::Received {
          info: identify::Info { observed_addr, .. },
          ..
        }) = &event
        {
          swarm.add_external_address(observed_addr.clone());
        }

        println!("{event:?}")
      }
      SwarmEvent::NewListenAddr { address, .. } => {
        println!("Listening on {address:?}");
      }
      _ => {}
    }
  }
}

#[derive(NetworkBehaviour)]
struct Behaviour {
  relay: relay::Behaviour,
  ping: ping::Behaviour,
  identify: identify::Behaviour,
}

fn generate_ed25519(secret_key_seed: u8) -> identity::Keypair {
  let mut bytes = [0u8; 32];
  bytes[0] = secret_key_seed;

  identity::Keypair::ed25519_from_bytes(bytes).expect("only errors on wrong length")
}

#[derive(Debug, Parser)]
#[clap(name = "libp2p relay")]
struct Opt {
  /// Determine if the relay listen on ipv6 or ipv4 loopback address. the default is ipv4
  #[clap(long)]
  use_ipv6: Option<bool>,

  /// Fixed value to generate deterministic peer id
  #[clap(long)]
  secret_key_seed: u8,

  /// The port used to listen on all interfaces
  #[clap(long)]
  port: u16,
}
