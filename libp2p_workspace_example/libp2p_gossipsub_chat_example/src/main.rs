use std::{
  collections::hash_map::DefaultHasher,
  error::Error,
  hash::{Hash, Hasher},
  time::Duration,
};

use futures::stream::StreamExt;
use libp2p::{
  gossipsub,
  gossipsub::partial_messages::{Metadata, Partial, PartialAction, PartialError},
  mdns, noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux, PeerId,
};
use tokio::{io, io::AsyncBufReadExt, select};
use tracing_subscriber::EnvFilter;

const TEXT_PART_COUNT: usize = 2;

// We create a custom network behaviour that combines Gossipsub and Mdns.
#[derive(NetworkBehaviour)]
struct MyBehaviour {
  gossipsub: gossipsub::Behaviour,
  mdns: mdns::tokio::Behaviour,
}

#[derive(Clone, Debug)]
struct TextPartialMessage {
  group_id: Vec<u8>,
  parts: [Vec<u8>; TEXT_PART_COUNT],
  available: u8,
}

impl TextPartialMessage {
  fn new(group_id: Vec<u8>, data: Vec<u8>) -> Self {
    let split_at = (data.len() + 1) / 2;
    let first = data[.. split_at].to_vec();
    let second = data[split_at ..].to_vec();

    Self {
      group_id,
      parts: [first, second],
      available: 0b11,
    }
  }

  fn group_id_for(peer_id: &PeerId, sequence: u64) -> Vec<u8> {
    let mut group_id = peer_id.to_bytes();
    group_id.extend_from_slice(&sequence.to_be_bytes());
    group_id
  }

  fn encode_body(&self, bitmap: u8) -> Vec<u8> {
    let mut body = vec![bitmap];

    for index in 0 .. TEXT_PART_COUNT {
      if bitmap & (1 << index) == 0 {
        continue;
      }

      let part = &self.parts[index];
      body.extend_from_slice(&(part.len() as u32).to_be_bytes());
      body.extend_from_slice(part);
    }

    body
  }

  fn decode_body(mut body: &[u8]) -> Result<Vec<(usize, Vec<u8>)>, String> {
    if body.is_empty() {
      return Err("missing partial bitmap".into());
    }

    let bitmap = body[0];
    body = &body[1 ..];

    let mut parts = Vec::new();
    for index in 0 .. TEXT_PART_COUNT {
      if bitmap & (1 << index) == 0 {
        continue;
      }

      if body.len() < 4 {
        return Err("missing partial length".into());
      }

      let mut len_bytes = [0; 4];
      len_bytes.copy_from_slice(&body[.. 4]);
      let part_len = u32::from_be_bytes(len_bytes) as usize;
      body = &body[4 ..];

      if body.len() < part_len {
        return Err("partial body is shorter than declared length".into());
      }

      parts.push((index, body[.. part_len].to_vec()));
      body = &body[part_len ..];
    }

    if !body.is_empty() {
      return Err("partial body has trailing bytes".into());
    }

    Ok(parts)
  }
}

impl Partial for TextPartialMessage {
  fn group_id(&self) -> Vec<u8> {
    self.group_id.clone()
  }

  fn metadata(&self) -> Box<dyn Metadata> {
    Box::new(TextPartialMetadata::new(self.available))
  }

  fn partial_action_from_metadata(
    &self,
    _peer_id: PeerId,
    metadata: Option<&[u8]>,
  ) -> Result<PartialAction, PartialError> {
    let peer_available = match metadata {
      Some(metadata) if metadata.len() == 1 => metadata[0],
      Some(_) => return Err(PartialError::InvalidFormat),
      None => 0,
    };

    let missing_for_peer = self.available & !peer_available;
    let peer_has_useful_data = peer_available & !self.available != 0;

    if missing_for_peer == 0 {
      return Ok(PartialAction {
        need: peer_has_useful_data,
        send: None,
      });
    }

    Ok(PartialAction {
      need: peer_has_useful_data,
      send: Some((
        self.encode_body(missing_for_peer),
        Box::new(TextPartialMetadata::new(peer_available | missing_for_peer)),
      )),
    })
  }
}

#[derive(Debug)]
struct TextPartialMetadata {
  bitmap: [u8; 1],
}

impl TextPartialMetadata {
  fn new(bitmap: u8) -> Self {
    Self { bitmap: [bitmap] }
  }
}

impl Metadata for TextPartialMetadata {
  fn as_slice(&self) -> &[u8] {
    &self.bitmap
  }

  fn update(&mut self, data: &[u8]) -> Result<bool, PartialError> {
    if data.len() != 1 {
      return Err(PartialError::InvalidFormat);
    }

    let before = self.bitmap[0];
    self.bitmap[0] |= data[0];
    Ok(before != self.bitmap[0])
  }

  fn update_from_data(&mut self, data: &[u8]) -> Result<(), PartialError> {
    if data.is_empty() {
      return Err(PartialError::InvalidFormat);
    }

    self.bitmap[0] |= data[0];
    Ok(())
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let mut swarm = libp2p::SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_quic()
    .with_behaviour(|key| {
      // To content-address message, we can take the hash of message and use it as an ID.
      let message_id_fn = |message: &gossipsub::Message| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        gossipsub::MessageId::from(s.finish().to_string())
      };

      // Set a custom gossipsub configuration
      let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
        .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
        .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
        .build()
        .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?; // Temporary hack because `build` does not return a proper `std::error::Error`.

      // build a gossipsub network behaviour
      let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(key.clone()),
        gossipsub_config,
      )?;

      let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;
      Ok(MyBehaviour { gossipsub, mdns })
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
    .build();

  // Create a Gossipsub topic
  let topic = gossipsub::IdentTopic::new("test-net");
  // Register topics with enabled partial messages.
  swarm
    .behaviour_mut()
    .gossipsub
    .enable_partials_for_topic(topic.hash(), true);
  // subscribes to our topic
  swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
  let local_peer_id = *swarm.local_peer_id();
  let mut partial_sequence = 0u64;

  // Read full lines from stdin
  let mut stdin = io::BufReader::new(io::stdin()).lines();

  // Listen on all interfaces and whatever port the OS assigns
  swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

  println!("Enter messages via STDIN and they will be sent to connected peers using Gossipsub");

  // Kick it off
  loop {
    select! {
      Ok(Some(line)) = stdin.next_line() => {
        partial_sequence += 1;
        let line_bytes = line.into_bytes();
        let partial = TextPartialMessage::new(
          TextPartialMessage::group_id_for(&local_peer_id, partial_sequence),
          line_bytes.clone(),
        );

        if let Err(e) = swarm
          .behaviour_mut().gossipsub
          .publish_partial(topic.hash(), partial) {
            println!("Partial publish error: {e:?}; falling back to full publish");
            if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), line_bytes) {
              println!("Publish error: {e:?}");
            }
        }
      }
      event = swarm.select_next_some() => match event {
        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
          for (peer_id, _multiaddr) in list {
            println!("mDNS discovered a new peer: {peer_id}");
            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
          }
        },
        SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
          for (peer_id, _multiaddr) in list {
            println!("mDNS discover peer has expired: {peer_id}");
            swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
          }
        },
        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
          propagation_source: peer_id,
          message_id: id,
          message,
        })) => println!(
          "Got message: '{}' with id: {id} from peer: {peer_id}",
          String::from_utf8_lossy(&message.data),
        ),
        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Partial {
          topic_hash,
          peer_id,
          group_id,
          message,
          metadata,
        })) => {
          println!(
            "Got partial message on {topic_hash} from {peer_id}: group={group_id:?}, \
             metadata={metadata:?}"
          );

          if let Some(body) = message {
            match TextPartialMessage::decode_body(&body) {
              Ok(parts) => {
                for (index, bytes) in parts {
                  println!(
                    "  part {index}: '{}'",
                    String::from_utf8_lossy(&bytes)
                  );
                }
              }
              Err(error) => println!("  failed to decode partial body: {error}"),
            }
          }
        },
        SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed {
          peer_id,
          topic,
          supports_partial,
          requests_partial,
        })) => println!(
          "Peer {peer_id} subscribed to {topic}; supports_partial={supports_partial}, \
           requests_partial={requests_partial}"
        ),
        SwarmEvent::NewListenAddr { address, .. } => {
          println!("Local node is listening on {address}");
        }
        _ => {}
      }
    }
  }
}
