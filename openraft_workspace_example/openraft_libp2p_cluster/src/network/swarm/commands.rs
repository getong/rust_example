//! Swarm loop command surface: the `Command` enum sent over the mpsc
//! channels and the handlers that apply commands to the owned `Swarm`.

use std::collections::HashSet;

use libp2p::{Multiaddr, PeerId, Swarm, gossipsub, kad, request_response::ResponseChannel};
use tokio::sync::oneshot;

use super::{
  Behaviour, NetErr,
  dial::{add_kad_address_from_p2p, dial_peer_addr, ensure_peer_connection, leave_kad},
  state::{GetProvidersState, SwarmState},
};
use crate::network::{
  openraft_sync::{
    OPENRAFT_SYNC_AVAILABLE_TOPIC, OpenRaftSnapshotPartial, OpenRaftSyncState, group_id_string,
    sync_topic_hash,
  },
  rpc::{UnifiedRpcRequest, UnifiedRpcResponse},
};

pub enum Command {
  SetKadMode {
    mode: kad::Mode,
  },
  StartProviding {
    key: String,
    resp: oneshot::Sender<Result<(), NetErr>>,
  },
  LeaveKad {
    provider_keys: Vec<String>,
    resp: oneshot::Sender<Result<(), NetErr>>,
  },
  GetProviders {
    key: String,
    resp: oneshot::Sender<Result<HashSet<PeerId>, NetErr>>,
  },
  Dial {
    addr: Multiaddr,
  },
  EnsureConnection {
    peer: PeerId,
    addr: Multiaddr,
    resp: oneshot::Sender<Result<(), NetErr>>,
  },
  EnsureConnectionAny {
    peer: PeerId,
    resp: oneshot::Sender<Result<(), NetErr>>,
  },
  GossipsubPublish {
    topic: String,
    data: Vec<u8>,
  },
  GetLibp2pInfo {
    resp: oneshot::Sender<Libp2pSwarmReport>,
  },
  /// Publish an openraft snapshot partial that the caller already built
  /// off-loop (building can take seconds for a large state machine, see
  /// `Libp2pClient::publish_openraft_snapshot`), so publishing never stalls
  /// swarm event processing.
  PublishOpenRaftSnapshotBuilt {
    partial: OpenRaftSnapshotPartial,
    resp: oneshot::Sender<Result<String, NetErr>>,
  },
  RpcRequest {
    peer: PeerId,
    req: UnifiedRpcRequest,
    resp: oneshot::Sender<Result<UnifiedRpcResponse, NetErr>>,
  },
  RpcRespond {
    channel: ResponseChannel<UnifiedRpcResponse>,
    resp: UnifiedRpcResponse,
  },
}

/// Live gossipsub state for one subscribed topic.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GossipTopicReport {
  pub topic: String,
  /// Peers in this node's gossipsub mesh for the topic.
  pub mesh_peers: usize,
  /// Known peers subscribed to the topic (mesh + fanout candidates).
  pub subscribed_peers: usize,
}

/// Snapshot of the live libp2p swarm state, collected inside the swarm loop.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Libp2pSwarmReport {
  pub local_peer_id: String,
  /// Actual listening multiaddrs (after port/interface resolution).
  pub listeners: Vec<String>,
  pub connected_peers: usize,
  pub established_connections: u32,
  /// Kademlia mode: "Server" (control nodes advertise) or "Client" (workers).
  pub kad_mode: String,
  /// Peers currently in the kademlia routing table.
  pub kad_routing_table_peers: usize,
  pub gossipsub_topics: Vec<GossipTopicReport>,
  /// All peers the gossipsub behaviour knows about.
  pub gossipsub_known_peers: usize,
}

pub(crate) fn handle_command_batch(
  swarm: &mut Swarm<Behaviour>,
  cmd_batch: &mut Vec<Command>,
  state: &mut SwarmState,
) {
  for cmd in cmd_batch.drain(..) {
    match cmd {
      Command::PublishOpenRaftSnapshotBuilt { partial, resp } => {
        publish_openraft_snapshot_partial(swarm, partial, resp, &mut state.openraft_sync);
      }
      cmd => handle_command(swarm, cmd, state),
    }
  }
}

pub(crate) fn handle_command(swarm: &mut Swarm<Behaviour>, cmd: Command, state: &mut SwarmState) {
  match cmd {
    Command::SetKadMode { mode } => {
      swarm.behaviour_mut().kad.set_mode(Some(mode));
      tracing::info!(mode = ?mode, "set kademlia mode");
    }
    Command::StartProviding { key, resp } => {
      let record_key = kad::RecordKey::new(&key);
      match swarm.behaviour_mut().kad.start_providing(record_key) {
        Ok(query_id) => {
          state.pending_kad.start_providing.insert(query_id, resp);
        }
        Err(e) => {
          let _ = resp.send(Err(NetErr(format!("start_providing failed: {:?}", e))));
        }
      }
    }
    Command::LeaveKad {
      provider_keys,
      resp,
    } => {
      leave_kad(swarm, &provider_keys);
      let _ = resp.send(Ok(()));
    }
    Command::GetProviders { key, resp } => {
      let record_key = kad::RecordKey::new(&key);
      let query_id = swarm.behaviour_mut().kad.get_providers(record_key);
      state.pending_kad.get_providers.insert(
        query_id,
        GetProvidersState {
          providers: HashSet::new(),
          resp,
        },
      );
    }
    Command::Dial { addr } => {
      dial_peer_addr(swarm, addr.clone());
      // Inserting the address triggers kad's automatic (throttled)
      // bootstrap; no manual query kick is needed.
      add_kad_address_from_p2p(swarm, &addr);
    }
    Command::EnsureConnection { peer, addr, resp } => {
      ensure_peer_connection(
        swarm,
        &mut state.pending_connect,
        &mut state.dial_backoff_until,
        peer,
        Some(addr),
        resp,
      );
    }
    Command::EnsureConnectionAny { peer, resp } => {
      ensure_peer_connection(
        swarm,
        &mut state.pending_connect,
        &mut state.dial_backoff_until,
        peer,
        None,
        resp,
      );
    }
    Command::GossipsubPublish { topic, data } => {
      let topic = gossipsub::IdentTopic::new(topic);
      if let Err(err) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
        tracing::warn!("gossipsub publish failed: {err}");
      }
    }
    Command::GetLibp2pInfo { resp } => {
      let _ = resp.send(collect_swarm_report(swarm));
    }
    Command::PublishOpenRaftSnapshotBuilt { resp, .. } => {
      let _ = resp.send(Err(NetErr(
        "openraft snapshot sync is not available in this swarm loop".to_string(),
      )));
    }
    Command::RpcRequest { peer, req, resp } => {
      let id = swarm.behaviour_mut().rpc.send_request(&peer, req);
      state.pending_rpc.insert(id, resp);
    }
    Command::RpcRespond { channel, resp } => {
      let _ = swarm.behaviour_mut().rpc.send_response(channel, resp);
    }
  }
}

pub(crate) fn publish_openraft_snapshot_partial(
  swarm: &mut Swarm<Behaviour>,
  partial: OpenRaftSnapshotPartial,
  resp: oneshot::Sender<Result<String, NetErr>>,
  openraft_sync: &mut OpenRaftSyncState,
) {
  let sync_group = group_id_string(&partial.group_id);
  let topic = sync_topic_hash();
  let publish_result = swarm
    .behaviour_mut()
    .gossipsub
    .publish_partial(topic, partial.clone());

  match publish_result {
    Ok(()) => {
      // Best-effort "snapshot available" broadcast so peers that missed the
      // partial push (joined the mesh late, dropped messages) learn the
      // snapshot exists and can advertise their need bitmap to pull it.
      match partial.available_announcement().encode() {
        Ok(data) => {
          let topic = gossipsub::IdentTopic::new(OPENRAFT_SYNC_AVAILABLE_TOPIC);
          if let Err(err) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
            tracing::debug!(error = ?err, "publish snapshot-available announcement failed");
          }
        }
        Err(err) => {
          tracing::warn!(error = ?err, "encode snapshot-available announcement failed");
        }
      }
      openraft_sync.insert_local(partial);
      let _ = resp.send(Ok(sync_group));
    }
    Err(err) => {
      let _ = resp.send(Err(NetErr(format!(
        "publish openraft snapshot partial failed: {err}"
      ))));
    }
  }
}

pub(crate) fn collect_swarm_report(swarm: &mut Swarm<Behaviour>) -> Libp2pSwarmReport {
  let local_peer_id = swarm.local_peer_id().to_string();
  let listeners: Vec<String> = swarm.listeners().map(|addr| addr.to_string()).collect();
  let network_info = swarm.network_info();
  let connected_peers = network_info.num_peers();
  let established_connections = network_info.connection_counters().num_established();

  let behaviour = swarm.behaviour_mut();
  let kad_mode = format!("{:?}", behaviour.kad.mode());
  let kad_routing_table_peers: usize = behaviour
    .kad
    .kbuckets()
    .map(|bucket| bucket.num_entries())
    .sum();

  // Per-topic mesh and subscription counts.
  let topics: Vec<gossipsub::TopicHash> = behaviour.gossipsub.topics().cloned().collect();
  let peer_topics: Vec<Vec<gossipsub::TopicHash>> = behaviour
    .gossipsub
    .all_peers()
    .map(|(_, topics)| topics.into_iter().cloned().collect())
    .collect();
  let gossipsub_known_peers = peer_topics.len();
  let gossipsub_topics = topics
    .iter()
    .map(|hash| GossipTopicReport {
      topic: hash.to_string(),
      mesh_peers: behaviour.gossipsub.mesh_peers(hash).count(),
      subscribed_peers: peer_topics
        .iter()
        .filter(|topics| topics.contains(hash))
        .count(),
    })
    .collect();

  Libp2pSwarmReport {
    local_peer_id,
    listeners,
    connected_peers,
    established_connections,
    kad_mode,
    kad_routing_table_peers,
    gossipsub_topics,
    gossipsub_known_peers,
  }
}
