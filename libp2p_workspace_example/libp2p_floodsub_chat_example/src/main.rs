use futures::StreamExt;
use libp2p::{
  // core::transport::upgrade::Version,
  floodsub::{self, Floodsub, FloodsubEvent},
  identity,
  mdns,

  noise,
  // swarm::{dial_opts::DialOpts, NetworkBehaviourEventProcess, SwarmBuilder, SwarmEvent},
  swarm::{NetworkBehaviour, SwarmEvent},
  // `TokioTcpConfig` is available through the `tcp-tokio` feature.
  tcp,
  yamux,
  Multiaddr,
  PeerId,
  SwarmBuilder,
  // Transport,
};
use std::error::Error;
use tokio::io::{self, AsyncBufReadExt};
use tokio::time::Duration;
// We create a custom network behaviour that combines floodsub and mDNS.
// The derive generates a delegating `NetworkBehaviour` impl which in turn
// requires the implementations of `NetworkBehaviourEventProcess` for
// the events of each behaviour.
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "MyBehaviourEvent")]
struct MyBehaviour {
  floodsub: Floodsub,
  mdns: mdns::tokio::Behaviour,
}

pub enum MyBehaviourEvent {
  FloodSub(FloodsubEvent),
  Mdns(mdns::Event),
}

impl From<FloodsubEvent> for MyBehaviourEvent {
  fn from(event: FloodsubEvent) -> Self {
    MyBehaviourEvent::FloodSub(event)
  }
}

impl From<mdns::Event> for MyBehaviourEvent {
  fn from(event: mdns::Event) -> Self {
    MyBehaviourEvent::Mdns(event)
  }
}

// impl NetworkBehaviourEventProcess<FloodsubEvent> for MyBehaviour {
//     // Called when `floodsub` produces an event.
//     fn inject_event(&mut self, message: FloodsubEvent) {
//         if let FloodsubEvent::Message(message) = message {
//             println!(
//                 "Received: '{:?}' from {:?}",
//                 String::from_utf8_lossy(&message.data),
//                 message.source
//             );
//         }
//     }
// }

// impl NetworkBehaviourEventProcess<MdnsEvent> for MyBehaviour {
//     // Called when `mdns` produces an event.
//     fn inject_event(&mut self, event: MdnsEvent) {
//         match event {
//             MdnsEvent::Discovered(list) => {
//                 for (peer, _) in list {
//                     self.floodsub.add_node_to_partial_view(peer);
//                 }
//             }
//             MdnsEvent::Expired(list) => {
//                 for (peer, _) in list {
//                     if !self.mdns.has_node(&peer) {
//                         self.floodsub.remove_node_from_partial_view(&peer);
//                     }
//                 }
//             }
//         }
//     }
// }

/// The `tokio::main` attribute sets up a tokio runtime.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  env_logger::init();

  // Create a random PeerId
  let id_keys = identity::Keypair::generate_ed25519();
  // let peer_id = PeerId::from(id_keys.public());
  // println!("Local peer id: {:?}", peer_id);

  // // Create a keypair for authenticated encryption of the transport.
  // let noise = noise::Config::new(&id_keys).unwrap();

  // // Create a tokio-based TCP transport use noise for authenticated
  // // encryption and Mplex for multiplexing of substreams on a TCP stream.
  // let transport = tcp::tokio::Transport::default()
  //   .upgrade(Version::V1Lazy)
  //   .authenticate(noise)
  //   .multiplex(yamux::Config::default())
  //   .boxed();

  // Create a Floodsub topic
  let floodsub_topic = floodsub::Topic::new("chat");

  // Create a Swarm to manage peers and events.
  // let mut swarm = {
  //   let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id).unwrap();
  //   let mut behaviour = MyBehaviour {
  //     floodsub: Floodsub::new(peer_id),
  //     mdns,
  //   };

  //   behaviour.floodsub.subscribe(floodsub_topic.clone());

  //   SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build()
  // };
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
      let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id).unwrap();
      MyBehaviour {
        floodsub: Floodsub::new(peer_id),
        mdns,
      }
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
    .build();

  swarm
    .behaviour_mut()
    .floodsub
    .subscribe(floodsub_topic.clone());

  // Reach out to another node if specified
  if let Some(to_dial) = std::env::args().nth(1) {
    let addr: Multiaddr = to_dial.parse()?;
    swarm.dial(addr)?;
    println!("Dialed {:?}", to_dial);
  }

  // Read full lines from stdin
  let mut stdin = io::BufReader::new(io::stdin()).lines();

  // Listen on all interfaces and whatever port the OS assigns
  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

  // Kick it off
  loop {
    tokio::select! {
        line = stdin.next_line() => {
            let line = line?.expect("stdin closed");
            swarm.behaviour_mut().floodsub.publish(floodsub_topic.clone(), line);
        }
        event = swarm.select_next_some() => {
            if let SwarmEvent::NewListenAddr { address, .. } = event {
                println!("Listening on {:?}", address);
            }
        }
    }
  }
}
