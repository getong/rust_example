use std::{env, error::Error, fs, path::Path, str::FromStr, time::Duration};

use either::Either;
use futures::prelude::*;
use libp2p::{
  core::transport::upgrade::Version,
  gossipsub,
  gossipsub::partial_messages::{Metadata, Partial, PartialAction, PartialError},
  identify,
  multiaddr::Protocol,
  noise, ping,
  pnet::{PnetConfig, PreSharedKey},
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux, Multiaddr, PeerId, Transport,
};
use tokio::{io, io::AsyncBufReadExt, select};
use tracing_subscriber::EnvFilter;

const TEXT_PART_COUNT: usize = 2;

#[derive(NetworkBehaviour)]
struct MyBehaviour {
  gossipsub: gossipsub::Behaviour,
  identify: identify::Behaviour,
  ping: ping::Behaviour,
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

/// Get the current ipfs repo path, either from the IPFS_PATH environment variable or
/// from the default $HOME/.ipfs
fn get_ipfs_path() -> Box<Path> {
  env::var("IPFS_PATH")
    .map(|ipfs_path| Path::new(&ipfs_path).into())
    .unwrap_or_else(|_| {
      env::var("HOME")
        .map(|home| Path::new(&home).join(".ipfs"))
        .expect("could not determine home directory")
        .into()
    })
}

/// Read the pre shared key file from the given ipfs directory
fn get_psk(path: &Path) -> std::io::Result<Option<String>> {
  let swarm_key_file = path.join("swarm.key");
  match fs::read_to_string(swarm_key_file) {
    Ok(text) => Ok(Some(text)),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(e) => Err(e),
  }
}

/// for a multiaddr that ends with a peer id, this strips this suffix. Rust-libp2p
/// only supports dialing to an address without providing the peer id.
fn strip_peer_id(addr: &mut Multiaddr) {
  let last = addr.pop();
  match last {
    Some(Protocol::P2p(peer_id)) => {
      let mut addr = Multiaddr::empty();
      addr.push(Protocol::P2p(peer_id));
      println!("removing peer id {addr} so this address can be dialed by rust-libp2p");
    }
    Some(other) => addr.push(other),
    _ => {}
  }
}

/// parse a legacy multiaddr (replace ipfs with p2p), and strip the peer id
/// so it can be dialed by rust-libp2p
fn parse_legacy_multiaddr(text: &str) -> Result<Multiaddr, Box<dyn Error>> {
  let sanitized = text
    .split('/')
    .map(|part| if part == "ipfs" { "p2p" } else { part })
    .collect::<Vec<_>>()
    .join("/");
  let mut res = Multiaddr::from_str(&sanitized)?;
  strip_peer_id(&mut res);
  Ok(res)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let _ = tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .try_init();

  let ipfs_path = get_ipfs_path();
  println!("using IPFS_PATH {ipfs_path:?}");
  let psk: Option<PreSharedKey> = get_psk(&ipfs_path)?
    .map(|text| PreSharedKey::from_str(&text))
    .transpose()?;

  if let Some(psk) = psk {
    println!("using swarm key with fingerprint: {}", psk.fingerprint());
  }

  // Create a Gosspipsub topic
  let gossipsub_topic = gossipsub::IdentTopic::new("chat");

  // We create a custom network behaviour that combines gossipsub, ping and identify.

  let mut swarm = libp2p::SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_other_transport(|key| {
      let noise_config = noise::Config::new(key).unwrap();
      let yamux_config = yamux::Config::default();

      let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
      let maybe_encrypted = match psk {
        Some(psk) => Either::Left(
          base_transport.and_then(move |socket, _| PnetConfig::new(psk).handshake(socket)),
        ),
        None => Either::Right(base_transport),
      };
      maybe_encrypted
        .upgrade(Version::V1Lazy)
        .authenticate(noise_config)
        .multiplex(yamux_config)
    })?
    .with_dns()?
    .with_behaviour(|key| {
      let gossipsub_config = gossipsub::ConfigBuilder::default()
        .max_transmit_size(262144)
        .build()
        .map_err(|msg| io::Error::new(io::ErrorKind::Other, msg))?; // Temporary hack because `build` does not return a proper `std::error::Error`.
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
        ping: ping::Behaviour::new(ping::Config::new()),
      })
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
    .build();

  println!("Subscribing to {gossipsub_topic:?}");
  // Register topics with enabled partial messages.
  swarm
    .behaviour_mut()
    .gossipsub
    .enable_partials_for_topic(gossipsub_topic.hash(), true);
  swarm
    .behaviour_mut()
    .gossipsub
    .subscribe(&gossipsub_topic)
    .unwrap();
  let local_peer_id = *swarm.local_peer_id();
  let mut partial_sequence = 0u64;

  // Reach out to other nodes if specified
  for to_dial in std::env::args().skip(1) {
    let addr: Multiaddr = parse_legacy_multiaddr(&to_dial)?;
    swarm.dial(addr)?;
    println!("Dialed {to_dial:?}")
  }

  // Read full lines from stdin
  let mut stdin = io::BufReader::new(io::stdin()).lines();

  // Listen on all interfaces and whatever port the OS assigns
  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

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
                .behaviour_mut()
                .gossipsub
                .publish_partial(gossipsub_topic.hash(), partial)
            {
                println!("Partial publish error: {e:?}; falling back to full publish");
                if let Err(e) = swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(gossipsub_topic.clone(), line_bytes)
                {
                    println!("Publish error: {e:?}");
                }
            }
        },
        event = swarm.select_next_some() => {
            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {address:?}");
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Identify(event)) => {
                    println!("identify: {event:?}");
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                    propagation_source: peer_id,
                    message_id: id,
                    message,
                })) => {
                    println!(
                        "Got message: {} with id: {} from peer: {:?}",
                        String::from_utf8_lossy(&message.data),
                        id,
                        peer_id
                    )
                }
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
                }
                SwarmEvent::Behaviour(MyBehaviourEvent::Gossipsub(gossipsub::Event::Subscribed {
                    peer_id,
                    topic,
                    supports_partial,
                    requests_partial,
                })) => println!(
                    "Peer {peer_id} subscribed to {topic}; supports_partial={supports_partial}, \
                     requests_partial={requests_partial}"
                ),
                SwarmEvent::Behaviour(MyBehaviourEvent::Ping(event)) => {
                    match event {
                        ping::Event {
                            peer,
                            result: Result::Ok(rtt),
                            ..
                        } => {
                            println!(
                                "ping: rtt to {} is {} ms",
                                peer.to_base58(),
                                rtt.as_millis()
                            );
                        }
                        ping::Event {
                            peer,
                            result: Result::Err(ping::Failure::Timeout),
                            ..
                        } => {
                            println!("ping: timeout to {}", peer.to_base58());
                        }
                        ping::Event {
                            peer,
                            result: Result::Err(ping::Failure::Unsupported),
                            ..
                        } => {
                            println!("ping: {} does not support ping protocol", peer.to_base58());
                        }
                        ping::Event {
                            peer,
                            result: Result::Err(ping::Failure::Other { error }),
                            ..
                        } => {
                            println!("ping: ping::Failure with {}: {error}", peer.to_base58());
                        }
                    }
                }
                _ => {}
            }
        }
    }
  }
}

// mkdir -p ~/.ipfs
// openssl rand -base64 32 > ~/.ipfs/swarm.key
