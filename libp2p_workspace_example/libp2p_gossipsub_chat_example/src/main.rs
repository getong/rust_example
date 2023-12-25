use env_logger::{Builder, Env};
use futures::prelude::*;
use libp2p::gossipsub::MessageId;
use libp2p::gossipsub::{IdentTopic as Topic, Message, ValidationMode};
use libp2p::{
  // core::transport::upgrade::Version,
  gossipsub,
  identify,
  identity,
  noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp,
  yamux,
  Multiaddr,
  // PeerId,
  SwarmBuilder,
};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::io::{self, AsyncBufReadExt};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
  gossipsub: gossipsub::Behaviour,
  identify: identify::Behaviour,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  Builder::from_env(Env::default().default_filter_or("info")).init();

  // Create a random PeerId
  let local_key = identity::Keypair::generate_ed25519();
  // let local_peer_id = PeerId::from(local_key.public());
  // println!("Local peer id: {:?}", local_peer_id);

  // let noise = noise::Config::new(&local_key).unwrap();
  // // Set up an encrypted TCP Transport over the Mplex and Yamux protocols
  // // let transport = libp2p::development_transport(local_key.clone()).await?;
  // let transport = tcp::tokio::Transport::default()
  //   .upgrade(Version::V1Lazy)
  //   .authenticate(noise)
  //   .multiplex(yamux::Config::default())
  //   .boxed();

  // Create a Gossipsub topic
  let topic = Topic::new("test-net");

  // Create a Swarm to manage peers and events
  // let mut swarm = {
  //   // To content-address message, we can take the hash of message and use it as an ID.
  //   let message_id_fn = |message: &Message| {
  //     let mut s = DefaultHasher::new();
  //     message.data.hash(&mut s);
  //     MessageId::from(s.finish().to_string())
  //   };

  //   // Set a custom gossipsub
  //   let gossipsub_config = gossipsub::ConfigBuilder::default()
  //     .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
  //     .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
  //     .message_id_fn(message_id_fn) // content-address messages. No two messages of the
  //     // same content will be propagated.
  //     .build()
  //     .expect("Valid config");
  //   // build a gossipsub network behaviour
  //   let mut gossipsub: gossipsub::Behaviour =
  //     gossipsub::Behaviour::new(MessageAuthenticity::Signed(local_key), gossipsub_config)
  //       .expect("Correct configuration");

  //   // subscribes to our topic
  //   gossipsub.subscribe(&topic).unwrap();
  let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_dns()?
    .with_behaviour(|key| {
      // let peer_id = PeerId::from(key.public());
      // ChatBehavior::new(peer_id).unwrap()
      let message_id_fn = |message: &Message| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        MessageId::from(s.finish().to_string())
      };
      let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
        .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
        .message_id_fn(message_id_fn) // content-address messages. No two messages of the
        // same content will be propagated.
        .build()
        .expect("Valid config");
      // build a gossipsub network behaviour
      Ok(MyBehaviour {
        gossipsub: gossipsub::Behaviour::new(
          gossipsub::MessageAuthenticity::Signed(key.clone()),
          gossipsub_config,
        )
        .expect("Valid configuration"),
        identify: identify::Behaviour::new(identify::Config::new(
          "/ipfs/0.1.0".into(),
          key.public(),
        )),
      })
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
    .build();

  // add an explicit peer if one was provided
  if let Some(explicit) = std::env::args().nth(2) {
    // let explicit = explicit;
    match explicit.parse() {
      Ok(id) => swarm.behaviour_mut().gossipsub.add_explicit_peer(&id),
      Err(err) => println!("Failed to parse explicit peer id: {:?}", err),
    }
  }

  // build the swarm
  // libp2p::Swarm::new(transport, gossipsub, local_peer_id)
  // SwarmBuilder::with_tokio_executor(transport, gossipsub, local_peer_id).build()
  // };

  // Listen on all interfaces and whatever port the OS assigns
  swarm
    .listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
    .unwrap();

  // Reach out to another node if specified
  if let Some(to_dial) = std::env::args().nth(1) {
    let address: Multiaddr = to_dial.parse().expect("User to provide valid address.");
    match swarm.dial(address.clone()) {
      Ok(_) => println!("Dialed {:?}", address),
      Err(e) => println!("Dial {:?} failed: {:?}", address, e),
    };
  }

  // Read full lines from stdin
  // let mut stdin = io::BufReader::new(io::stdin()).lines().fuse();
  let mut stdin = io::BufReader::new(io::stdin()).lines();

  // Kick it off
  loop {
    tokio::select! {
        // line = stdin.select_next_some() => {
        line = stdin.next_line() => {
            let line = line?.expect("stdin closed");
            if let Err(e) = swarm
            .behaviour_mut()
            .gossipsub
                .publish(topic.clone(), line.as_bytes())
            {
                println!("Publish error: {:?}", e);
            }
        },
        event = swarm.select_next_some() => match event {
          SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                propagation_source: peer_id,
                message_id: id,
                message,
            })) => println!(
                "Got message: {} with id: {} from peer: {:?}",
                String::from_utf8_lossy(&message.data),
                id,
                peer_id
            ),
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("Listening on {:?}", address);
            }
            _ => {}
        }
    }
  }
}
