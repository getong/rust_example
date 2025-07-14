use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use chitchat::{
  Chitchat, ChitchatConfig, ChitchatHandle, ChitchatId, ClusterStateSnapshot,
  FailureDetectorConfig, spawn_chitchat, transport::UdpTransport,
};
use itertools::Itertools;
use openraft::{BasicNode, Raft, error::InitializeError};
use tokio::sync::{Mutex, RwLock};

use crate::distributed::{
  member::{Member, Service},
  network::ChitchatRaftNetwork,
  raft_types::{NodeId, TypeConfig},
  store::StateMachineStore,
};

const CLUSTER_ID: &str = "chitchat-example-cluster";
const GOSSIP_INTERVAL: Duration = Duration::from_secs(1);
const SERVICE_KEY: &str = "service";

type Result<T> = std::result::Result<T, anyhow::Error>;

fn snapshot_members(snapshot: ClusterStateSnapshot) -> Vec<Member> {
  let mut res = Vec::new();
  for state in snapshot.node_states {
    if let Some(service) = state.get(SERVICE_KEY) {
      if let Ok(service) = serde_json::from_str(service) {
        res.push(Member::with_id(
          state.chitchat_id().node_id.clone(),
          service,
        ));
      }
    }
  }
  res
}

pub struct Cluster {
  self_node: Option<Member>,
  chitchat: Arc<Mutex<Chitchat>>,
  // dropping the handle leaves the cluster
  _chitchat_handle: ChitchatHandle,
  // OpenRAFT integration
  raft: Arc<RwLock<Option<Raft<TypeConfig>>>>,
  raft_store: Arc<RwLock<Option<Arc<StateMachineStore>>>>,
}

impl Cluster {
  pub async fn join(
    mut self_node: Member,
    gossip_addr: SocketAddr,
    seed_addrs: Vec<SocketAddr>,
  ) -> Result<Self> {
    let failure_detector_config = FailureDetectorConfig {
      dead_node_grace_period: Duration::from_secs(10),
      ..Default::default()
    };

    let uuid = uuid::Uuid::new_v4().to_string();
    let node_id_string = format!("{}_{}", self_node.id, uuid);

    self_node.id = node_id_string.clone();

    let chitchat_id = ChitchatId::new(
      node_id_string,
      0, // generation
      gossip_addr,
    );

    let config = ChitchatConfig {
      cluster_id: CLUSTER_ID.to_string(),
      chitchat_id,
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
        serde_json::to_string(&self_node.service)?,
      )],
      Some(self_node),
    )
    .await
  }

  pub async fn join_as_spectator(
    gossip_addr: SocketAddr,
    seed_addrs: Vec<SocketAddr>,
  ) -> Result<Self> {
    let failure_detector_config = FailureDetectorConfig {
      dead_node_grace_period: Duration::from_secs(10),
      ..Default::default()
    };

    let uuid = uuid::Uuid::new_v4().to_string();
    let node_id_string = format!("{}_{}", CLUSTER_ID, uuid);

    let chitchat_id = ChitchatId::new(
      node_id_string,
      0, // generation
      gossip_addr,
    );

    let config = ChitchatConfig {
      cluster_id: CLUSTER_ID.to_string(),
      chitchat_id,
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
    self_node: Option<Member>,
  ) -> Result<Self> {
    let transport = UdpTransport;

    let chitchat_handle = spawn_chitchat(config, key_values, &transport).await?;
    let chitchat = chitchat_handle.chitchat();

    Ok(Self {
      self_node,
      chitchat,
      _chitchat_handle: chitchat_handle,
      raft: Arc::new(RwLock::new(None)),
      raft_store: Arc::new(RwLock::new(None)),
    })
  }

  pub async fn members(&self) -> Vec<Member> {
    snapshot_members(self.chitchat.lock().await.state_snapshot())
      .into_iter()
      .unique_by(|m| m.service.clone())
      .collect()
  }

  pub async fn await_member<P>(&self, pred: P) -> Member
  where
    P: Fn(&Member) -> bool,
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

  pub fn self_node(&self) -> Option<&Member> {
    self.self_node.as_ref()
  }

  /// Update service information using chitchat and sync with OpenRAFT
  pub async fn set_service(&self, service: Service) -> Result<()> {
    self
      .chitchat
      .lock()
      .await
      .self_node_state()
      .set(SERVICE_KEY, &serde_json::to_string(&service)?);

    tracing::debug!("Updated service information in chitchat: {}", service);
    Ok(())
  }

  /// Initialize OpenRAFT integration for this cluster
  pub async fn enable_raft(&self, node_id_str: String) -> Result<()> {
    if self.raft.read().await.is_some() {
      return Ok(()); // Already initialized
    }

    // Convert string node_id to u64 (simple hash for demo)
    let node_id: NodeId = node_id_str.len() as u64;

    // Create OpenRAFT configuration
    let raft_config = openraft::Config::default();
    let raft_config = Arc::new(raft_config.validate()?);

    // Create log store, state machine store, and network
    let log_store = crate::distributed::log_store::LogStore::default();
    let state_machine_store = Arc::new(StateMachineStore::default());
    let network = ChitchatRaftNetwork::new();

    // Initialize the Raft instance
    let raft = Raft::new(
      node_id,
      raft_config,
      network,
      log_store,
      state_machine_store.clone(),
    )
    .await?;

    // Initialize as single-node cluster (can be extended later for multi-node)
    let members: BTreeMap<NodeId, BasicNode> =
      BTreeMap::from([(node_id, BasicNode::new(format!("127.0.0.1:8080")))]);

    if let Err(e) = raft.initialize(members.clone()).await {
      match e {
        openraft::error::RaftError::APIError(e) => match e {
          InitializeError::NotAllowed(_) => {
            // Already initialized, that's fine
          }
          InitializeError::NotInMembers(_) => return Err(e.into()),
        },
        openraft::error::RaftError::Fatal(_) => return Err(e.into()),
      }
    }

    // Store the initialized components
    let mut raft_guard = self.raft.write().await;
    *raft_guard = Some(raft);

    let mut store_guard = self.raft_store.write().await;
    *store_guard = Some(state_machine_store);

    tracing::info!(
      "OpenRAFT integration enabled for cluster with node_id: {}",
      node_id
    );
    Ok(())
  }

  /// Get reference to the Raft instance if enabled
  pub async fn raft(&self) -> Option<Raft<TypeConfig>> {
    self.raft.read().await.clone()
  }

  /// Get reference to the state machine store if enabled
  pub async fn raft_store(&self) -> Option<Arc<StateMachineStore>> {
    self.raft_store.read().await.clone()
  }

  /// Execute a distributed operation using OpenRAFT
  pub async fn raft_request(
    &self,
    request: crate::distributed::raft_types::Request,
  ) -> Result<crate::distributed::raft_types::Response> {
    if let Some(raft) = self.raft().await {
      // Submit the request to OpenRAFT
      match raft.client_write(request).await {
        Ok(response) => Ok(response.data),
        Err(e) => Err(anyhow::anyhow!("Raft request failed: {:?}", e)),
      }
    } else {
      Err(anyhow::anyhow!("OpenRAFT not enabled"))
    }
  }

  pub async fn cluster_state(&self) -> ClusterStateSnapshot {
    self.chitchat.lock().await.state_snapshot()
  }

  pub async fn live_nodes(&self) -> Vec<ChitchatId> {
    self.chitchat.lock().await.live_nodes().cloned().collect()
  }

  pub async fn dead_nodes(&self) -> Vec<ChitchatId> {
    self.chitchat.lock().await.dead_nodes().cloned().collect()
  }

  #[cfg(test)]
  pub async fn remove_service(&self) -> Result<()> {
    self
      .chitchat
      .lock()
      .await
      .self_node_state()
      .set(SERVICE_KEY, "");

    Ok(())
  }
}
