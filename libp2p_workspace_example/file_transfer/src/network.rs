use std::{
  hash::{DefaultHasher, Hash, Hasher},
  time::Duration,
};

use libp2p::{
  Multiaddr, StreamProtocol, autonat, dcutr, gossipsub,
  gossipsub::{MessageAuthenticity, ValidationMode},
  identify,
  kad::{self, store::MemoryStore},
  mdns::{self, tokio::Tokio},
  multiaddr::Protocol,
  noise,
  ping::{self, Config},
  relay,
  request_response::{self, json},
  swarm::{NetworkBehaviour, Swarm, behaviour::toggle::Toggle},
  tcp, yamux,
};
use serde::{Deserialize, Serialize};

pub(crate) const FILE_TOPIC: &str = "file-transfer";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct MessageRequest {
  pub message: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct MessageResponse {
  pub ack: bool,
}

#[derive(NetworkBehaviour)]
pub(crate) struct ChatBehavior {
  pub(crate) ping: ping::Behaviour,
  pub(crate) messaging: json::Behaviour<MessageRequest, MessageResponse>,
  pub(crate) mdns: Toggle<mdns::Behaviour<Tokio>>,
  pub(crate) identify: identify::Behaviour,
  pub(crate) kademlia: kad::Behaviour<MemoryStore>,
  pub(crate) autonat: autonat::Behaviour,
  pub(crate) relay_server: relay::Behaviour,
  pub(crate) relay_client: relay::client::Behaviour,
  pub(crate) dcutr: dcutr::Behaviour,
  pub(crate) gossipsub: gossipsub::Behaviour,
}

pub(crate) fn build_swarm(mdns_enabled: bool) -> anyhow::Result<Swarm<ChatBehavior>> {
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

  Ok(swarm)
}

pub(crate) fn add_bootstrap_peers(
  swarm: &mut Swarm<ChatBehavior>,
  bootstrap_peers: Vec<String>,
) -> anyhow::Result<()> {
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

  Ok(())
}
