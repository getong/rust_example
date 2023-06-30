use futures::StreamExt;
use libp2p::{
    core::transport::upgrade::Version,
    identify, identity, noise, ping, rendezvous,
    swarm::{keep_alive, NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use std::time::Duration;

#[tokio::main]
async fn main() {
    env_logger::init();

    let key_pair = identity::Keypair::generate_ed25519();
    let rendezvous_point_address = "/ip4/127.0.0.1/tcp/62649".parse::<Multiaddr>().unwrap();
    let rendezvous_point = "12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN"
        .parse()
        .unwrap();

    let mut swarm = SwarmBuilder::with_tokio_executor(
        tcp::tokio::Transport::default()
            .upgrade(Version::V1Lazy)
            .authenticate(noise::Config::new(&key_pair).unwrap())
            .multiplex(yamux::Config::default())
            .boxed(),
        MyBehaviour {
            identify: identify::Behaviour::new(identify::Config::new(
                "rendezvous-example/1.0.0".to_string(),
                key_pair.public(),
            )),
            rendezvous: rendezvous::client::Behaviour::new(key_pair.clone()),
            ping: ping::Behaviour::new(ping::Config::new().with_interval(Duration::from_secs(1))),
            keep_alive: keep_alive::Behaviour,
        },
        PeerId::from(key_pair.public()),
    )
    .build();

    log::info!("Local peer id: {}", swarm.local_peer_id());

    let _ = swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap());

    swarm.dial(rendezvous_point_address.clone()).unwrap();

    while let Some(event) = swarm.next().await {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                log::info!("Listening on {}", address);
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                cause: Some(error),
                ..
            } if peer_id == rendezvous_point => {
                log::error!("Lost connection to rendezvous point {}", error);
            }
            // once `/identify` did its job, we know our external address and can register
            SwarmEvent::Behaviour(MyBehaviourEvent::Identify(identify::Event::Received {
                ..
            })) => {
                if let Err(error) = swarm.behaviour_mut().rendezvous.register(
                    rendezvous::Namespace::from_static("rendezvous"),
                    rendezvous_point,
                    None,
                ) {
                    log::error!("Failed to register: {error}");
                    return;
                }
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Rendezvous(
                rendezvous::client::Event::Registered {
                    namespace,
                    ttl,
                    rendezvous_node,
                },
            )) => {
                log::info!(
                    "Registered for namespace '{}' at rendezvous point {} for the next {} seconds",
                    namespace,
                    rendezvous_node,
                    ttl
                );
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Rendezvous(
                rendezvous::client::Event::RegisterFailed {
                    rendezvous_node,
                    namespace,
                    error,
                },
            )) => {
                log::error!(
                    "Failed to register: rendezvous_node={}, namespace={}, error_code={:?}",
                    rendezvous_node,
                    namespace,
                    error
                );
                return;
            }
            SwarmEvent::Behaviour(MyBehaviourEvent::Ping(ping::Event {
                peer,
                result: Ok(rtt),
                ..
            })) if peer != rendezvous_point => {
                log::info!("Ping to {} is {}ms", peer, rtt.as_millis())
            }
            other => {
                log::debug!("Unhandled {:?}", other);
            }
        }
    }
}

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    identify: identify::Behaviour,
    rendezvous: rendezvous::client::Behaviour,
    ping: ping::Behaviour,
    keep_alive: keep_alive::Behaviour,
}