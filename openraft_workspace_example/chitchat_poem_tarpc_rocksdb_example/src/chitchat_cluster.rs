// Stract-inspired chitchat cluster implementation
// Based on: https://github.com/StractOrg/stract/blob/main/crates/core/src/distributed/cluster.rs

use std::{net::SocketAddr, sync::Arc, time::Duration};

use chitchat::{
  spawn_chitchat, transport::UdpTransport, Chitchat, ChitchatConfig, ChitchatHandle, ChitchatId,
  ClusterStateSnapshot, FailureDetectorConfig,
};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{Node, NodeId as RaftNodeId};

const CLUSTER_ID: &str = "chitchat-raft-cluster";
const GOSSIP_INTERVAL: Duration = Duration::from_secs(1);
const SERVICE_KEY: &str = "service";

type Result<T> = std::result::Result<T, anyhow::Error>;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum ServiceType {
  RaftNode {
    api_addr: String,
    rpc_addr: String,
    raft_id: RaftNodeId,
  },
  SearchEngine {
    api_addr: String,
    shard_id: u64,
  },
  Gateway {
    api_addr: String,
  },
}

impl std::fmt::Display for ServiceType {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::RaftNode {
        api_addr,
        rpc_addr,
        raft_id,
      } => {
        write!(f, "RaftNode {} {} {}", api_addr, rpc_addr, raft_id)
      }
      Self::SearchEngine { api_addr, shard_id } => {
        write!(f, "SearchEngine {} {}", api_addr, shard_id)
      }
      Self::Gateway { api_addr } => {
        write!(f, "Gateway {}", api_addr)
      }
    }
  }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct ClusterMember {
  pub id: String,
  pub service: ServiceType,
}

impl ClusterMember {
  pub fn new(service: ServiceType) -> Self {
    let id = uuid::Uuid::new_v4().to_string();
    Self { id, service }
  }

  pub fn is_raft_node(&self) -> bool {
    matches!(self.service, ServiceType::RaftNode { .. })
  }

  pub fn get_raft_node_info(&self) -> Option<(String, String, RaftNodeId)> {
    match &self.service {
      ServiceType::RaftNode {
        api_addr,
        rpc_addr,
        raft_id,
      } => Some((api_addr.clone(), rpc_addr.clone(), *raft_id)),
      _ => None,
    }
  }
}

fn snapshot_members(snapshot: ClusterStateSnapshot) -> Vec<ClusterMember> {
  let mut res = Vec::new();
  for node_state in snapshot.node_states {
    if let Some(service) = node_state.get(SERVICE_KEY) {
      if let Ok(service) = serde_json::from_str(service) {
        res.push(ClusterMember {
          service,
          id: node_state.chitchat_id().node_id.clone(),
        });
      }
    }
  }
  res
}

pub struct ChitchatCluster {
  self_member: Option<ClusterMember>,
  chitchat: Arc<Mutex<Chitchat>>,
  // dropping the handle leaves the cluster
  _chitchat_handle: ChitchatHandle,
}

impl ChitchatCluster {
  pub async fn join(
    mut self_member: ClusterMember,
    gossip_addr: SocketAddr,
    seed_addrs: Vec<SocketAddr>,
  ) -> Result<Self> {
    let failure_detector_config = FailureDetectorConfig {
      initial_interval: GOSSIP_INTERVAL,
      ..Default::default()
    };

    let uuid = uuid::Uuid::new_v4().to_string();

    let chitchat_id = ChitchatId {
      node_id: format!("{}_{}", self_member.id, uuid),
      generation_id: 0,
      gossip_advertise_addr: gossip_addr,
    };
    self_member.id = chitchat_id.node_id.clone();

    let config = ChitchatConfig {
      chitchat_id,
      cluster_id: CLUSTER_ID.to_string(),
      gossip_interval: GOSSIP_INTERVAL,
      listen_addr: gossip_addr,
      seed_nodes: seed_addrs
        .into_iter()
        .map(|addr| addr.to_string())
        .collect(),
      failure_detector_config,
      marked_for_deletion_grace_period: Duration::from_secs(60),
      catchup_callback: None,
      extra_liveness_predicate: None,
    };

    Self::join_with_config(
      config,
      vec![(
        SERVICE_KEY.to_string(),
        serde_json::to_string(&self_member.service)?,
      )],
      Some(self_member),
    )
    .await
  }

  pub async fn join_as_spectator(
    gossip_addr: SocketAddr,
    seed_addrs: Vec<SocketAddr>,
  ) -> Result<Self> {
    let failure_detector_config = FailureDetectorConfig {
      initial_interval: GOSSIP_INTERVAL,
      ..Default::default()
    };

    let uuid = uuid::Uuid::new_v4().to_string();

    let chitchat_id = ChitchatId {
      node_id: format!("{}_{}", CLUSTER_ID, uuid),
      generation_id: 0,
      gossip_advertise_addr: gossip_addr,
    };

    let config = ChitchatConfig {
      chitchat_id,
      cluster_id: CLUSTER_ID.to_string(),
      gossip_interval: GOSSIP_INTERVAL,
      listen_addr: gossip_addr,
      seed_nodes: seed_addrs
        .into_iter()
        .map(|addr| addr.to_string())
        .collect(),
      failure_detector_config,
      marked_for_deletion_grace_period: Duration::from_secs(60),
      catchup_callback: None,
      extra_liveness_predicate: None,
    };

    Self::join_with_config(config, vec![], None).await
  }

  async fn join_with_config(
    config: ChitchatConfig,
    key_values: Vec<(String, String)>,
    self_member: Option<ClusterMember>,
  ) -> Result<Self> {
    let transport = UdpTransport;

    let chitchat_handle = spawn_chitchat(config, key_values, &transport).await?;
    let chitchat = chitchat_handle.chitchat();

    Ok(Self {
      self_member,
      chitchat,
      _chitchat_handle: chitchat_handle,
    })
  }

  pub async fn members(&self) -> Vec<ClusterMember> {
    snapshot_members(self.chitchat.lock().await.state_snapshot())
      .into_iter()
      .unique_by(|m| m.service.clone())
      .collect()
  }

  pub async fn raft_nodes(&self) -> Vec<ClusterMember> {
    self
      .members()
      .await
      .into_iter()
      .filter(|m| m.is_raft_node())
      .collect()
  }

  pub async fn await_member<P>(&self, pred: P) -> ClusterMember
  where
    P: Fn(&ClusterMember) -> bool,
  {
    loop {
      let members = self.members().await;
      for member in members {
        if pred(&member) {
          return member;
        }
      }

      tokio::time::sleep(Duration::from_secs(1)).await;
    }
  }

  pub async fn await_raft_nodes(&self, min_count: usize) -> Vec<ClusterMember> {
    loop {
      let raft_nodes = self.raft_nodes().await;
      if raft_nodes.len() >= min_count {
        return raft_nodes;
      }

      tokio::time::sleep(Duration::from_secs(1)).await;
    }
  }

  pub fn self_member(&self) -> Option<&ClusterMember> {
    self.self_member.as_ref()
  }

  pub async fn set_service(&self, service: ServiceType) -> Result<()> {
    self
      .chitchat
      .lock()
      .await
      .self_node_state()
      .set(SERVICE_KEY, serde_json::to_string(&service)?);

    Ok(())
  }

  /// Discover raft cluster configuration from chitchat members
  pub async fn discover_raft_cluster(&self) -> std::collections::BTreeMap<RaftNodeId, Node> {
    let mut raft_cluster = std::collections::BTreeMap::new();

    for member in self.raft_nodes().await {
      if let Some((api_addr, rpc_addr, raft_id)) = member.get_raft_node_info() {
        raft_cluster.insert(raft_id, Node { api_addr, rpc_addr });
      }
    }

    raft_cluster
  }

  #[cfg(test)]
  pub async fn remove_service(&self) -> Result<()> {
    self
      .chitchat
      .lock()
      .await
      .self_node_state()
      .set(SERVICE_KEY, String::new());

    Ok(())
  }
}
