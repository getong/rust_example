use std::{
  collections::hash_map::DefaultHasher,
  hash::{Hash, Hasher},
  time::Duration,
};

use anyhow::{Context, anyhow};
use libp2p::{
  Multiaddr, PeerId, Swarm,
  gossipsub::{self, IdentTopic},
  identity, mdns, noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux,
};
use openraft::ServerState;

use crate::{
  domain::{DistributedTask, TaskStage, TaskUnderstanding},
  journal::ConsensusTaskJournal,
  state::AppState,
};

const TASK_TOPIC: &str = "apalis/task/distribution/1";

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourEvent")]
pub(crate) struct Behaviour {
  gossipsub: gossipsub::Behaviour,
  mdns: mdns::tokio::Behaviour,
}

pub(crate) enum BehaviourEvent {
  Gossipsub(gossipsub::Event),
  Mdns(mdns::Event),
}

impl From<gossipsub::Event> for BehaviourEvent {
  fn from(event: gossipsub::Event) -> Self {
    Self::Gossipsub(event)
  }
}

impl From<mdns::Event> for BehaviourEvent {
  fn from(event: mdns::Event) -> Self {
    Self::Mdns(event)
  }
}

pub(crate) fn task_topic() -> IdentTopic {
  IdentTopic::new(TASK_TOPIC)
}

pub(crate) fn build_swarm(
  local_key: identity::Keypair,
  topic: &IdentTopic,
) -> anyhow::Result<Swarm<Behaviour>> {
  let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )
    .context("build tcp transport")?
    .with_quic()
    .with_behaviour(|key| {
      let local_peer_id = PeerId::from(key.public());
      let message_id_fn = |message: &gossipsub::Message| {
        let mut hasher = DefaultHasher::new();
        message.data.hash(&mut hasher);
        gossipsub::MessageId::from(hasher.finish().to_string())
      };
      let gossipsub_config = gossipsub::ConfigBuilder::default()
        .validation_mode(gossipsub::ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .build()
        .map_err(|e| anyhow!("gossipsub config: {e}"))?;
      let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(key.clone()),
        gossipsub_config,
      )?;
      let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;
      Ok(Behaviour { gossipsub, mdns })
    })
    .context("build libp2p behaviour")?
    .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
    .build();

  swarm
    .behaviour_mut()
    .gossipsub
    .subscribe(topic)
    .context("subscribe task topic")?;

  Ok(swarm)
}

pub(crate) async fn publish_task(
  swarm: &mut Swarm<Behaviour>,
  topic: &IdentTopic,
  journal: &ConsensusTaskJournal,
  task: &DistributedTask,
) -> anyhow::Result<()> {
  journal
    .append(TaskUnderstanding::new(
      task,
      TaskStage::Published,
      *swarm.local_peer_id(),
      "task published onto libp2p gossipsub",
    ))
    .await?;

  let payload = serde_json::to_vec(task).context("encode distributed task")?;
  if let Err(err) = swarm
    .behaviour_mut()
    .gossipsub
    .publish(topic.clone(), payload)
  {
    tracing::warn!(
      task_id = %task.id,
      error = %err,
      "task was recorded locally but not broadcast to any peer"
    );
  }

  Ok(())
}

pub(crate) async fn handle_swarm_event(
  event: SwarmEvent<BehaviourEvent>,
  swarm: &mut Swarm<Behaviour>,
  topic: &IdentTopic,
  state: &AppState,
) -> anyhow::Result<()> {
  match event {
    SwarmEvent::NewListenAddr { address, .. } => {
      println!("listening: {address}/p2p/{}", swarm.local_peer_id());
    }
    SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Discovered(peers))) => {
      for (peer_id, addr) in peers {
        println!("discovered peer {peer_id} at {addr}");
        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
      }
    }
    SwarmEvent::Behaviour(BehaviourEvent::Mdns(mdns::Event::Expired(peers))) => {
      for (peer_id, _) in peers {
        swarm
          .behaviour_mut()
          .gossipsub
          .remove_explicit_peer(&peer_id);
      }
    }
    SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gossipsub::Event::Message {
      message, ..
    })) => {
      if message.topic != topic.hash() {
        return Ok(());
      }

      let task: DistributedTask =
        serde_json::from_slice(&message.data).context("decode distributed task")?;
      state
        .enqueue_task_once(
          task,
          TaskStage::ReceivedFromLibp2p,
          "task received from libp2p and evaluated against openraft role before apalis enqueue",
        )
        .await?;
    }
    _ => {}
  }

  Ok(())
}

pub(crate) async fn report_ready_address(
  peer_id: PeerId,
  requested_listen: Multiaddr,
  external_addresses: Vec<Multiaddr>,
  openraft_node_id: String,
  openraft_state: Option<ServerState>,
  openraft_leader: Option<String>,
) {
  tokio::time::sleep(Duration::from_millis(300)).await;
  println!("local peer id: {peer_id}");
  println!("openraft node id: {openraft_node_id}");
  match openraft_state {
    Some(state) => println!("openraft state: {state:?}"),
    None => println!("openraft state: <unknown>"),
  }
  match openraft_leader {
    Some(leader) => println!("openraft current leader: {leader}"),
    None => println!("openraft current leader: <unknown>"),
  }
  println!("requested listen: {requested_listen}");
  for addr in external_addresses {
    println!("external address: {addr}/p2p/{peer_id}");
  }
}

pub(crate) fn swarm_external_addresses(swarm: &Swarm<Behaviour>) -> Vec<Multiaddr> {
  swarm.external_addresses().cloned().collect()
}
