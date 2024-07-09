// copy from libp2p/example/autonat

use clap::Parser;
use futures::StreamExt;
use libp2p::core::multiaddr::Protocol;
use libp2p::core::Multiaddr;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
use libp2p::{autonat, identify, identity, noise, tcp, yamux, PeerId};
use std::error::Error;
use std::net::Ipv4Addr;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[clap(name = "libp2p autonat")]
struct Opt {
  #[clap(long)]
  listen_port: Option<u16>,

  #[clap(long)]
  server_address: Multiaddr,

  #[clap(long)]
  server_peer_id: PeerId,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let opt = Opt::parse();

  let mut swarm = libp2p::SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_behaviour(|key| Behaviour::new(key.public()))?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
    .build();

  swarm.listen_on(
    Multiaddr::empty()
      .with(Protocol::Ip4(Ipv4Addr::UNSPECIFIED))
      .with(Protocol::Tcp(opt.listen_port.unwrap_or(0))),
  )?;

  swarm
    .behaviour_mut()
    .auto_nat
    .add_server(opt.server_peer_id, Some(opt.server_address));

  loop {
    match swarm.select_next_some().await {
      SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {address:?}"),
      SwarmEvent::Behaviour(event) => println!("{event:?}"),
      e => println!("{e:?}"),
    }
  }
}

#[derive(NetworkBehaviour)]
struct Behaviour {
  identify: identify::Behaviour,
  auto_nat: autonat::Behaviour,
}

impl Behaviour {
  fn new(local_public_key: identity::PublicKey) -> Self {
    Self {
      identify: identify::Behaviour::new(identify::Config::new(
        "/ipfs/0.1.0".into(),
        local_public_key.clone(),
      )),
      auto_nat: autonat::Behaviour::new(
        local_public_key.to_peer_id(),
        autonat::Config {
          retry_interval: Duration::from_secs(10),
          refresh_interval: Duration::from_secs(30),
          boot_delay: Duration::from_secs(5),
          throttle_server_period: Duration::ZERO,
          only_global_ips: false,
          ..Default::default()
        },
      ),
    }
  }
}
