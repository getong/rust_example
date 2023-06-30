// copy from libp2p/example/autonat

use clap::Parser;
use futures::prelude::*;
use libp2p::core::multiaddr::Protocol;
use libp2p::core::{upgrade::Version, Multiaddr, Transport};
use libp2p::swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent};
use libp2p::{autonat, identify, identity, noise, tcp, yamux, PeerId};
use std::error::Error;
use std::net::Ipv4Addr;
use std::time::Duration;

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
    env_logger::init();

    let opt = Opt::parse();

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Local peer id: {local_peer_id:?}");

    let transport = tcp::tokio::Transport::default()
        .upgrade(Version::V1Lazy)
        .authenticate(noise::Config::new(&local_key)?)
        .multiplex(yamux::Config::default())
        .boxed();

    let behaviour = Behaviour::new(local_key.public());

    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
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

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
enum Event {
    AutoNat(autonat::Event),
    Identify(identify::Event),
}

impl From<identify::Event> for Event {
    fn from(v: identify::Event) -> Self {
        Self::Identify(v)
    }
}

impl From<autonat::Event> for Event {
    fn from(v: autonat::Event) -> Self {
        Self::AutoNat(v)
    }
}
