use std::{
  collections::{HashMap, HashSet},
  env,
  hash::{DefaultHasher, Hash, Hasher},
  io::ErrorKind,
  path::{Path, PathBuf},
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

use libp2p::{
  Multiaddr, PeerId, StreamProtocol, autonat, dcutr,
  futures::StreamExt,
  gossipsub::{
    self, IdentTopic, MessageAuthenticity, ValidationMode,
    partial_messages::{Metadata, Partial, PartialAction, PartialError},
  },
  identify,
  kad::{self, store::MemoryStore},
  mdns::{self, tokio::Tokio},
  multiaddr::Protocol,
  noise,
  ping::{self, Config},
  relay,
  request_response::{self, json},
  swarm::{NetworkBehaviour, SwarmEvent, behaviour::toggle::Toggle},
  tcp, yamux,
};
use serde::{Deserialize, Serialize};
use tokio::{
  io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
  select,
};

const FILE_TOPIC: &str = "file-transfer";
const PART_SIZE: usize = 8 * 1024;
const MAX_PARTS_PER_MESSAGE: usize = 4;
const GROUP_ID_LEN: usize = 8;
const MAX_PARTS: usize = u16::MAX as usize;
const METADATA_MAGIC: &[u8; 4] = b"FPM1";

static NEXT_PARTIAL_GROUP: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MessageRequest {
  pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MessageResponse {
  pub ack: bool,
}

#[derive(Clone, Debug)]
struct FileMetadata {
  file_name: String,
  file_size: u64,
  total_parts: u16,
  bitmap: Vec<u8>,
}

#[derive(Clone, Debug)]
struct FilePartialMessage {
  group_id: Vec<u8>,
  file_name: String,
  file_size: u64,
  parts: Vec<Option<Vec<u8>>>,
}

#[derive(Debug)]
enum FileWriteOutcome {
  Written(PathBuf),
  Duplicate(PathBuf),
}

impl FilePartialMessage {
  async fn from_path(path: &Path, local_peer_id: PeerId) -> anyhow::Result<Self> {
    let bytes = tokio::fs::read(path).await?;
    let total_parts = bytes.len().div_ceil(PART_SIZE).max(1);
    if total_parts > MAX_PARTS {
      anyhow::bail!("file needs {total_parts} parts; max supported is {MAX_PARTS}");
    }

    let file_name = path
      .file_name()
      .and_then(|name| name.to_str())
      .ok_or_else(|| anyhow::anyhow!("file path has no valid UTF-8 file name"))?
      .to_string();
    if file_name.len() > u16::MAX as usize {
      anyhow::bail!("file name is too long");
    }

    let sequence = NEXT_PARTIAL_GROUP.fetch_add(1, Ordering::Relaxed);
    let mut hasher = DefaultHasher::new();
    local_peer_id.hash(&mut hasher);
    file_name.hash(&mut hasher);
    bytes.hash(&mut hasher);
    sequence.hash(&mut hasher);
    let group_id = hasher.finish().to_be_bytes().to_vec();
    let parts = if bytes.is_empty() {
      vec![Some(Vec::new())]
    } else {
      bytes
        .chunks(PART_SIZE)
        .map(|chunk| Some(chunk.to_vec()))
        .collect()
    };

    Ok(Self {
      group_id,
      file_name,
      file_size: bytes.len() as u64,
      parts,
    })
  }

  fn empty(group_id: Vec<u8>, metadata: &FileMetadata) -> Self {
    Self {
      group_id,
      file_name: metadata.file_name.clone(),
      file_size: metadata.file_size,
      parts: vec![None; metadata.total_parts as usize],
    }
  }

  fn metadata_bytes(&self) -> Vec<u8> {
    let file_name = self.file_name.as_bytes();
    let mut metadata = Vec::with_capacity(16 + file_name.len() + self.bitmap_len());
    metadata.extend_from_slice(METADATA_MAGIC);
    metadata.extend_from_slice(&self.total_parts().to_be_bytes());
    metadata.extend_from_slice(&self.file_size.to_be_bytes());
    metadata.extend_from_slice(&(file_name.len() as u16).to_be_bytes());
    metadata.extend_from_slice(file_name);
    metadata.extend(self.bitmap());
    metadata
  }

  fn total_parts(&self) -> u16 {
    self.parts.len() as u16
  }

  fn bitmap_len(&self) -> usize {
    self.parts.len().div_ceil(8)
  }

  fn bitmap(&self) -> Vec<u8> {
    let mut bitmap = vec![0; self.bitmap_len()];
    for (index, part) in self.parts.iter().enumerate() {
      if part.is_some() {
        bitmap[index / 8] |= 1 << (index % 8);
      }
    }
    bitmap
  }

  fn has_part(bitmap: &[u8], index: usize) -> bool {
    bitmap
      .get(index / 8)
      .map(|byte| byte & (1 << (index % 8)) != 0)
      .unwrap_or(false)
  }

  fn merge_metadata(left: &mut [u8], right: &[u8]) -> Result<bool, PartialError> {
    let left_header_len = Self::metadata_header_len(left)?;
    let right_header_len = Self::metadata_header_len(right)?;
    if left_header_len != right_header_len
      || left.len() != right.len()
      || left[.. left_header_len] != right[.. right_header_len]
    {
      return Err(PartialError::InvalidFormat);
    }

    let mut updated = false;
    for (left, right) in left[left_header_len ..]
      .iter_mut()
      .zip(&right[right_header_len ..])
    {
      let merged = *left | *right;
      updated |= merged != *left;
      *left = merged;
    }
    Ok(updated)
  }

  fn metadata_matches(&self, metadata: &FileMetadata) -> bool {
    self.file_name == metadata.file_name
      && self.file_size == metadata.file_size
      && self.total_parts() == metadata.total_parts
  }

  fn metadata_header_len(metadata: &[u8]) -> Result<usize, PartialError> {
    if metadata.len() < 16 || &metadata[0 .. 4] != METADATA_MAGIC {
      return Err(PartialError::InvalidFormat);
    }

    let file_name_len = u16::from_be_bytes([metadata[14], metadata[15]]) as usize;
    let header_len = 16 + file_name_len;
    if metadata.len() < header_len {
      return Err(PartialError::InvalidFormat);
    }
    Ok(header_len)
  }

  fn encode_parts_for(
    &self,
    peer_metadata: Option<&[u8]>,
  ) -> Result<Option<Vec<u8>>, PartialError> {
    let requested = match peer_metadata {
      Some(metadata) => {
        let metadata = Self::parse_metadata(metadata)?;
        if !self.metadata_matches(&metadata) {
          return Err(PartialError::InvalidFormat);
        }
        metadata.bitmap
      }
      None => vec![0; self.bitmap_len()],
    };

    let mut body = Vec::new();
    let mut sent_parts = 0;
    for (index, part) in self.parts.iter().enumerate() {
      if Self::has_part(&requested, index) {
        continue;
      }
      let Some(part) = part else {
        continue;
      };
      body.extend_from_slice(&(index as u16).to_be_bytes());
      body.extend_from_slice(&(part.len() as u16).to_be_bytes());
      body.extend_from_slice(part);
      sent_parts += 1;

      if sent_parts >= MAX_PARTS_PER_MESSAGE {
        break;
      }
    }

    if body.is_empty() {
      Ok(None)
    } else {
      body.extend_from_slice(&self.total_parts().to_be_bytes());
      body.extend_from_slice(&self.group_id);
      Ok(Some(body))
    }
  }

  fn merge_body(&mut self, body: &[u8]) -> Result<bool, PartialError> {
    if body.len() < self.group_id.len() + 2 {
      return Err(PartialError::InvalidFormat);
    }

    let trailer_start = body.len() - self.group_id.len() - 2;
    let total_parts = u16::from_be_bytes([body[trailer_start], body[trailer_start + 1]]);
    let received_group_id = &body[trailer_start + 2 ..];
    if total_parts != self.total_parts() {
      return Err(PartialError::InvalidFormat);
    }
    if received_group_id != self.group_id {
      return Err(PartialError::WrongGroup {
        received: received_group_id.to_vec(),
      });
    }

    let mut offset = 0;
    let mut updated = false;
    while offset < trailer_start {
      if offset + 4 > trailer_start {
        return Err(PartialError::InvalidFormat);
      }

      let index = u16::from_be_bytes([body[offset], body[offset + 1]]) as usize;
      let len = u16::from_be_bytes([body[offset + 2], body[offset + 3]]) as usize;
      offset += 4;

      if index >= self.parts.len() || offset + len > trailer_start {
        return Err(PartialError::OutOfRange);
      }

      if self.parts[index].is_none() {
        self.parts[index] = Some(body[offset .. offset + len].to_vec());
        updated = true;
      }
      offset += len;
    }

    Ok(updated)
  }

  fn is_complete(&self) -> bool {
    self.parts.iter().all(Option::is_some)
  }

  fn to_bytes(&self) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(self.file_size as usize);
    for part in &self.parts {
      if let Some(part) = part {
        bytes.extend_from_slice(part);
      } else {
        anyhow::bail!("file is missing parts");
      }
    }

    if bytes.len() as u64 != self.file_size {
      anyhow::bail!(
        "assembled file size mismatch: expected {}, got {}",
        self.file_size,
        bytes.len()
      );
    }
    Ok(bytes)
  }

  fn output_path(content_hash: &str) -> PathBuf {
    PathBuf::from("received").join(content_hash)
  }

  async fn write_to_disk(&self) -> anyhow::Result<FileWriteOutcome> {
    let bytes = self.to_bytes()?;
    let content_hash = blake3::hash(&bytes).to_hex().to_string();
    let output_path = Self::output_path(&content_hash);

    tokio::fs::create_dir_all("received").await?;
    let mut file = match tokio::fs::OpenOptions::new()
      .write(true)
      .create_new(true)
      .open(&output_path)
      .await
    {
      Ok(file) => file,
      Err(error) if error.kind() == ErrorKind::AlreadyExists => {
        return Ok(FileWriteOutcome::Duplicate(output_path));
      }
      Err(error) => return Err(error.into()),
    };

    file.write_all(&bytes).await?;
    Ok(FileWriteOutcome::Written(output_path))
  }

  fn parse_metadata(metadata: &[u8]) -> Result<FileMetadata, PartialError> {
    let header_len = Self::metadata_header_len(metadata)?;

    let total_parts = u16::from_be_bytes([metadata[4], metadata[5]]);
    if total_parts == 0 {
      return Err(PartialError::InvalidFormat);
    }

    let file_size = u64::from_be_bytes([
      metadata[6],
      metadata[7],
      metadata[8],
      metadata[9],
      metadata[10],
      metadata[11],
      metadata[12],
      metadata[13],
    ]);
    let file_name = std::str::from_utf8(&metadata[16 .. header_len])
      .map_err(|_| PartialError::InvalidFormat)?
      .to_string();
    if file_name.is_empty() {
      return Err(PartialError::InvalidFormat);
    }

    let bitmap_len = (total_parts as usize).div_ceil(8);
    if metadata.len() != header_len + bitmap_len {
      return Err(PartialError::InvalidFormat);
    }

    Ok(FileMetadata {
      file_name,
      file_size,
      total_parts,
      bitmap: metadata[header_len ..].to_vec(),
    })
  }
}

impl Partial for FilePartialMessage {
  fn group_id(&self) -> Vec<u8> {
    self.group_id.clone()
  }

  fn metadata(&self) -> Box<dyn Metadata> {
    Box::new(FilePartialMetadata {
      bytes: self.metadata_bytes(),
    })
  }

  fn partial_action_from_metadata(
    &self,
    _peer_id: PeerId,
    metadata: Option<&[u8]>,
  ) -> Result<PartialAction, PartialError> {
    let peer_has_useful_data = if let Some(metadata) = metadata {
      let metadata = Self::parse_metadata(metadata)?;
      if !self.metadata_matches(&metadata) {
        return Err(PartialError::InvalidFormat);
      }
      self
        .parts
        .iter()
        .enumerate()
        .any(|(index, part)| part.is_none() && Self::has_part(&metadata.bitmap, index))
    } else {
      false
    };

    Ok(PartialAction {
      need: peer_has_useful_data,
      send: self
        .encode_parts_for(metadata)?
        .map(|body| (body, self.metadata())),
    })
  }
}

#[derive(Debug)]
struct FilePartialMetadata {
  bytes: Vec<u8>,
}

impl Metadata for FilePartialMetadata {
  fn as_slice(&self) -> &[u8] {
    &self.bytes
  }

  fn update(&mut self, data: &[u8]) -> Result<bool, PartialError> {
    FilePartialMessage::merge_metadata(&mut self.bytes, data)
  }

  fn update_from_data(&mut self, data: &[u8]) -> Result<(), PartialError> {
    let trailer_len = 2 + GROUP_ID_LEN;
    if data.len() < trailer_len {
      return Err(PartialError::InvalidFormat);
    }
    let payload_end = data.len() - trailer_len;
    let metadata = FilePartialMessage::parse_metadata(&self.bytes)?;
    let received_total_parts = u16::from_be_bytes([data[payload_end], data[payload_end + 1]]);
    if received_total_parts != metadata.total_parts {
      return Err(PartialError::InvalidFormat);
    }

    let mut update = vec![0; self.bytes.len()];
    let header_len = FilePartialMessage::metadata_header_len(&self.bytes)?;
    update[.. header_len].copy_from_slice(&self.bytes[.. header_len]);

    let mut offset = 0;
    while offset < payload_end {
      if offset + 4 > payload_end {
        return Err(PartialError::InvalidFormat);
      }
      let index = u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
      let len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
      offset += 4;
      if index >= metadata.total_parts as usize || offset + len > payload_end {
        return Err(PartialError::OutOfRange);
      }
      update[header_len + index / 8] |= 1 << (index % 8);
      offset += len;
    }

    FilePartialMessage::merge_metadata(&mut self.bytes, &update)?;
    Ok(())
  }
}

fn hex_id(bytes: &[u8]) -> String {
  bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[derive(NetworkBehaviour)]
struct ChatBehavior {
  ping: ping::Behaviour,
  messaging: json::Behaviour<MessageRequest, MessageResponse>,
  mdns: Toggle<mdns::Behaviour<Tokio>>,
  identify: identify::Behaviour,
  kademlia: kad::Behaviour<MemoryStore>,
  autonat: autonat::Behaviour,
  relay_server: relay::Behaviour,
  relay_client: relay::client::Behaviour,
  dcutr: dcutr::Behaviour,
  gossipsub: gossipsub::Behaviour,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let file_topic = IdentTopic::new(FILE_TOPIC);
  let mdns_enabled = env::var("CHAT_MDNS_ENABLED")?.parse::<bool>()?;
  let bootstrap_peers = env::var("CHAT_BOOTSTRAP_PEERS").map(|peers| {
    peers
      .split(',')
      .map(|s| s.to_string())
      .collect::<Vec<String>>()
  });

  let mut swarm = libp2p::SwarmBuilder::with_new_identity()
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_relay_client(noise::Config::new, yamux::Config::default)?
    .with_behaviour(|key_pair, relay_client| {
      let mdns = if mdns_enabled {
        Toggle::from(Some(mdns::Behaviour::new(
          mdns::Config::default(),
          key_pair.public().to_peer_id(),
        )?))
      } else {
        Toggle::from(None)
      };

      let mut kad_config = kad::Config::new(StreamProtocol::new("/awesome-chat/kad/1.0.0"));
      kad_config.set_periodic_bootstrap_interval(Some(Duration::from_secs(10)));

      let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10))
        .validation_mode(ValidationMode::Strict)
        .message_id_fn(|message| {
          let mut hasher = DefaultHasher::new();
          message.data.hash(&mut hasher);
          message.topic.hash(&mut hasher);
          if let Some(peer_id) = message.source {
            peer_id.hash(&mut hasher);
          }
          gossipsub::MessageId::from(hasher.finish().to_string())
        })
        .build()?;

      Ok(ChatBehavior {
        ping: ping::Behaviour::new(Config::new().with_interval(Duration::from_secs(10))),
        messaging: json::Behaviour::new(
          [(
            StreamProtocol::new("/awesome-chat/1"),
            request_response::ProtocolSupport::Full,
          )],
          request_response::Config::default(),
        ),
        mdns,
        identify: identify::Behaviour::new(identify::Config::new(
          "1.0.0".to_string(),
          key_pair.public(),
        )),
        kademlia: kad::Behaviour::with_config(
          key_pair.public().to_peer_id(),
          MemoryStore::new(key_pair.public().to_peer_id()),
          kad_config,
        ),
        autonat: autonat::Behaviour::new(
          key_pair.public().to_peer_id(),
          autonat::Config::default(),
        ),
        relay_server: relay::Behaviour::new(
          key_pair.public().to_peer_id(),
          relay::Config::default(),
        ),
        relay_client,
        dcutr: dcutr::Behaviour::new(key_pair.public().to_peer_id()),
        gossipsub: gossipsub::Behaviour::new(
          MessageAuthenticity::Signed(key_pair.clone()),
          gossipsub_config,
        )?,
      })
    })?
    .with_swarm_config(|config| config.with_idle_connection_timeout(Duration::from_secs(30)))
    .build();

  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
  swarm
    .behaviour_mut()
    .kademlia
    .set_mode(Some(kad::Mode::Server));

  // Initialize boostrap peers
  if let Ok(bootstrap_peers) = bootstrap_peers {
    for bootstrap_peer in bootstrap_peers {
      let addr: Multiaddr = bootstrap_peer.parse()?;
      let peer_id = addr
        .iter()
        .last()
        .and_then(|protocol| match protocol {
          Protocol::P2p(peer_id) => Some(peer_id),
          _ => None,
        })
        .ok_or(anyhow::anyhow!("No Peer ID found in address!"))?
        .to_owned();

      swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
      swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
    }
  }

  println!("Peer ID: {:?}", swarm.local_peer_id());

  println!("Commands:");
  println!("  /send <path>  send a file through gossipsub partial messages");
  println!("  /peers        print connected peers");
  println!("  /help         print commands");

  let file_topic_hash = file_topic.hash();
  swarm
    .behaviour_mut()
    .gossipsub
    .enable_partials_for_topic(file_topic_hash.clone(), true);
  swarm.behaviour_mut().gossipsub.subscribe(&file_topic)?;

  let mut stdin = BufReader::new(tokio::io::stdin()).lines();
  let mut file_messages: HashMap<Vec<u8>, FilePartialMessage> = HashMap::new();
  let mut completed_files: HashSet<Vec<u8>> = HashSet::new();

  loop {
    select! {
        event = swarm.select_next_some() => {
            match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on: {}", address);
                }
                SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                    println!("Connected to peer: {}", peer_id);
                }
                SwarmEvent::Behaviour(event) => match event {
                    ChatBehaviorEvent::Ping(_) => {}
                    ChatBehaviorEvent::Messaging(event) => match event {
                        request_response::Event::Message{message, peer, ..} => match message {
                            request_response::Message::Request {request, channel, ..} => {
                                println!("{peer} {:?}", request.message);
                                if let Err(err) = swarm.behaviour_mut().messaging.send_response(channel, MessageResponse { ack: true }) {
                                    println!("Error sending response: {:?}", err);
                                }
                            }
                            request_response::Message::Response {response, ..} => {
                                println!("{} Response ACK {response:?}", swarm.local_peer_id());
                            }
                        }
                        request_response::Event::ResponseSent{..} => {},
                        request_response::Event::OutboundFailure{peer, request_id, error, ..} => {
                            println!("Outbound failure: peer: {peer}, request_id: {request_id}, error: {error}");
                        },
                        request_response::Event::InboundFailure{peer, request_id, error, ..} => {
                            println!("Inbound failure: peer: {peer}, request_id: {request_id}, error: {error}");
                        },
                    }
                    ChatBehaviorEvent::Mdns(event) => match event {
                        mdns::Event::Discovered(new_peers) => {
                            for (peer_id, addr) in new_peers {
                                println!("Discovered peer: {}", peer_id);
                                swarm.add_peer_address(peer_id, addr.clone());
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            }
                        }
                        mdns::Event::Expired(_) => {
                        }
                    }
                    ChatBehaviorEvent::Identify(event) => match event {
                        identify::Event::Received{ peer_id, info, .. } => {
                            println!("New Identify received: {peer_id} - {info:?}");
                            let is_relay = info.protocols.iter().any(|protocol| {
                                *protocol == relay::HOP_PROTOCOL_NAME
                            });

                            for addr in info.listen_addrs {
                                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr.clone());
                                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);

                                if is_relay {
                                    let listen_addr = addr.clone().with_p2p(peer_id).unwrap().with(Protocol::P2pCircuit);
                                    println!("Trying to listen on {:?}", listen_addr);
                                    swarm.listen_on(listen_addr).unwrap();
                                }
                            }
                        }
                        identify::Event::Sent { .. } => {},
                        identify::Event::Pushed { .. } =>{},
                        identify::Event::Error { .. } =>{},
                    }
                    ChatBehaviorEvent::Kademlia(event) => match event {
                        kad::Event::InboundRequest { .. } => {}
                        kad::Event::OutboundQueryProgressed {..} => {}
                        kad::Event::RoutingUpdated{
                            addresses,
                            peer,
                            ..
                        } => {
                            println!("New Routing updated! Peer: {peer} - {addresses:?}");
                            addresses.iter().for_each(|addr| {
                                if let Err(err) = swarm.dial(addr.clone()) {
                                    println!("Error dialing address {addr}: {err}");
                                }
                            });
                        }
                        kad::Event::UnroutablePeer{..} => {}
                        kad::Event::RoutablePeer{..} => {}
                        kad::Event::PendingRoutablePeer{..} => {}
                        kad::Event::ModeChanged{..} => {}
                    }
                    ChatBehaviorEvent::Autonat(event) => match event {
                        autonat::Event::InboundProbe(event) => {
                            println!("Inbound probe: {event:?}");
                        }
                        autonat::Event::OutboundProbe(event) => {
                            println!("Outbound probe: {event:?}");
                        }
                        autonat::Event::StatusChanged{new, old} => {
                            println!("Status changed: {old:?} -> {new:?}");
                        }
                    }
                    ChatBehaviorEvent::RelayServer(event) => {
                        println!("Relay server: {event:?}");
                    }
                    ChatBehaviorEvent::RelayClient(event) => {
                        println!("Relay client: {event:?}");
                    }
                    ChatBehaviorEvent::Dcutr(event) => {
                        println!("Dcutr: {:?}", event);
                    },
                    ChatBehaviorEvent::Gossipsub(event) => match event {
                        gossipsub::Event::Message {
                            propagation_source,
                            message_id,
                            message,
                        } => {
                            println!(
                                "Gossipsub full message from {propagation_source} with id {message_id}: {:?}",
                                String::from_utf8_lossy(&message.data),
                            );
                        }
                        gossipsub::Event::Partial {
                            topic_hash,
                            peer_id,
                            group_id,
                            message,
                            metadata,
                        } => {
                            if topic_hash != file_topic_hash {
                                println!("Gossipsub partial on non-file topic {topic_hash}: {group_id:?}");
                                continue;
                            }

                            let Some(metadata) = metadata else {
                                println!("Ignoring partial from {peer_id}: missing metadata");
                                continue;
                            };

                            let remote_metadata = match FilePartialMessage::parse_metadata(&metadata) {
                                Ok(parsed) => parsed,
                                Err(error) => {
                                    println!("Invalid partial metadata from {peer_id}: {error}");
                                    swarm.behaviour_mut().gossipsub.report_invalid_partial(peer_id, &topic_hash);
                                    continue;
                                }
                            };

                            let partial = file_messages
                                .entry(group_id.clone())
                                .or_insert_with(|| FilePartialMessage::empty(group_id.clone(), &remote_metadata));

                            if !partial.metadata_matches(&remote_metadata) {
                                println!(
                                    "Ignoring mismatched file metadata from {peer_id}: group={}",
                                    hex_id(&group_id)
                                );
                                swarm.behaviour_mut().gossipsub.report_invalid_partial(peer_id, &topic_hash);
                                continue;
                            }

                            let mut updated = false;
                            if let Some(message) = message {
                                match partial.merge_body(&message) {
                                    Ok(part_updated) => updated = part_updated,
                                    Err(error) => {
                                        println!("Invalid partial body from {peer_id}: {error}");
                                        swarm.behaviour_mut().gossipsub.report_invalid_partial(peer_id, &topic_hash);
                                        continue;
                                    }
                                }
                            }

                            let remote_has_useful_data =
                                partial.parts.iter().enumerate().any(|(index, part)| {
                                    part.is_none()
                                        && FilePartialMessage::has_part(
                                            &remote_metadata.bitmap,
                                            index,
                                        )
                                });

                            if updated || remote_has_useful_data {
                                if let Err(error) = swarm
                                    .behaviour_mut()
                                    .gossipsub
                                    .publish_partial(topic_hash.clone(), partial.clone())
                                {
                                    println!("Error republishing partial file update: {error:?}");
                                }
                            }

                            if partial.is_complete() {
                                if completed_files.insert(group_id.clone()) {
                                    match partial.write_to_disk().await {
                                        Ok(FileWriteOutcome::Written(path)) => {
                                            println!(
                                                "Completed file from {peer_id}: {} ({} bytes, {} parts) -> {}",
                                                partial.file_name,
                                                partial.file_size,
                                                partial.total_parts(),
                                                path.display()
                                            );
                                        }
                                        Ok(FileWriteOutcome::Duplicate(path)) => {
                                            println!(
                                                "Completed duplicate file from {peer_id}: {} ({} bytes, {} parts) -> {} already exists, skipped",
                                                partial.file_name,
                                                partial.file_size,
                                                partial.total_parts(),
                                                path.display()
                                            );
                                        }
                                        Err(error) => {
                                            println!(
                                                "Failed to write completed file {} from {peer_id}: {error}",
                                                partial.file_name
                                            );
                                        }
                                    }
                                } else {
                                    println!(
                                        "Received duplicate complete file update from {peer_id}: {}",
                                        partial.file_name
                                    );
                                }
                            } else {
                                println!(
                                    "Received file partial from {peer_id}: {} group={} parts={}/{}",
                                    partial.file_name,
                                    hex_id(&group_id),
                                    partial.parts.iter().filter(|part| part.is_some()).count(),
                                    partial.total_parts()
                                );
                            }
                        }
                        gossipsub::Event::Subscribed {
                            peer_id,
                            topic,
                            supports_partial,
                            requests_partial,
                        } => {
                            println!(
                                "Gossipsub subscribed: {peer_id} topic={topic} supports_partial={supports_partial} requests_partial={requests_partial}"
                            );
                            if topic == file_topic_hash && supports_partial {
                                let known_files =
                                    file_messages.values().cloned().collect::<Vec<_>>();
                                for file in known_files {
                                    if let Err(error) = swarm
                                        .behaviour_mut()
                                        .gossipsub
                                        .publish_partial(topic.clone(), file)
                                    {
                                        println!(
                                            "Error advertising known file to new subscriber {peer_id}: {error:?}"
                                        );
                                    }
                                }
                            }
                        }
                        gossipsub::Event::Unsubscribed { peer_id, topic } => {
                            println!("Gossipsub unsubscribed: {peer_id} topic={topic}");
                        }
                        gossipsub::Event::GossipsubNotSupported { peer_id } => {
                            println!("Gossipsub not supported by {peer_id}");
                        }
                        gossipsub::Event::SlowPeer {
                            peer_id,
                            failed_messages,
                        } => {
                            println!("Gossipsub slow peer: {peer_id} failed_messages={failed_messages:?}");
                        }
                    }
                }
                _ => {}
            }
        },
        Ok(Some(line)) = stdin.next_line() => {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if line == "/help" {
                println!("Commands:");
                println!("  /send <path>  send a file through gossipsub partial messages");
                println!("  /peers        print connected peers");
                println!("  /help         print commands");
                continue;
            }

            if line == "/peers" {
                let peers = swarm
                    .connected_peers()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>();
                println!("Connected peers: {}", peers.join(", "));
                continue;
            }

            let Some(path) = line.strip_prefix("/send ") else {
                println!("Unknown command. Use /send <path>, /peers, or /help.");
                continue;
            };

            let path = PathBuf::from(path.trim());
            let local_peer_id = *swarm.local_peer_id();
            let partial = match FilePartialMessage::from_path(&path, local_peer_id).await {
                Ok(partial) => partial,
                Err(error) => {
                    println!("Failed to read file {}: {error}", path.display());
                    continue;
                }
            };
            match swarm
                .behaviour_mut()
                .gossipsub
                .publish_partial(file_topic_hash.clone(), partial.clone())
            {
                Ok(()) => {
                    println!(
                        "Started file transfer: {} ({} bytes, {} parts, group={})",
                        partial.file_name,
                        partial.file_size,
                        partial.total_parts(),
                        hex_id(&partial.group_id)
                    );
                }
                Err(error) => {
                    println!("Error publishing partial file: {:?}", error);
                }
            }
            completed_files.insert(partial.group_id.clone());
            file_messages.insert(partial.group_id.clone(), partial);
        }
    }
  }
}
