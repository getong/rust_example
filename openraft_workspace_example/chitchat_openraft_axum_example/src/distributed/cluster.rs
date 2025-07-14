use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};

use chitchat::{
  Chitchat, ChitchatConfig, ChitchatHandle, ChitchatId, ClusterStateSnapshot,
  FailureDetectorConfig, spawn_chitchat, transport::UdpTransport,
};
use itertools::Itertools;
use openraft::{BasicNode, Raft, error::InitializeError};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

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
    info!("Starting cluster join process for node: {}", self_node.id);
    debug!(
      "Gossip address: {}, Seed addresses: {:?}",
      gossip_addr, seed_addrs
    );

    let failure_detector_config = FailureDetectorConfig {
      dead_node_grace_period: Duration::from_secs(10),
      ..Default::default()
    };
    debug!(
      "Failure detector config: grace_period={:?}",
      failure_detector_config.dead_node_grace_period
    );

    let uuid = uuid::Uuid::new_v4().to_string();
    let node_id_string = format!("{}_{}", self_node.id, uuid);
    debug!("Generated UUID: {}", uuid);
    debug!("Generated node_id_string: {}", node_id_string);

    self_node.id = node_id_string.clone();
    debug!("Updated self_node.id to: {}", self_node.id);

    let chitchat_id = ChitchatId::new(
      node_id_string.clone(),
      0, // generation
      gossip_addr,
    );
    debug!(
      "Created chitchat_id: node_id={}, generation=0, addr={}",
      node_id_string, gossip_addr
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

    debug!(
      "Chitchat config created: cluster_id={}, gossip_interval={:?}, listen_addr={}, \
       seed_nodes={:?}",
      config.cluster_id, config.gossip_interval, config.listen_addr, config.seed_nodes
    );

    let service_json = serde_json::to_string(&self_node.service)?;
    debug!("Serialized service data: {}", service_json);

    info!("Joining cluster with config for node: {}", self_node.id);
    Self::join_with_config(
      config,
      vec![(SERVICE_KEY.to_string(), service_json)],
      Some(self_node),
    )
    .await
  }

  pub async fn join_as_spectator(
    gossip_addr: SocketAddr,
    seed_addrs: Vec<SocketAddr>,
  ) -> Result<Self> {
    info!("Starting spectator join process");
    debug!(
      "Spectator gossip address: {}, Seed addresses: {:?}",
      gossip_addr, seed_addrs
    );

    let failure_detector_config = FailureDetectorConfig {
      dead_node_grace_period: Duration::from_secs(10),
      ..Default::default()
    };
    debug!(
      "Failure detector config: grace_period={:?}",
      failure_detector_config.dead_node_grace_period
    );

    let uuid = uuid::Uuid::new_v4().to_string();
    let node_id_string = format!("{}_{}", CLUSTER_ID, uuid);
    debug!("Generated spectator UUID: {}", uuid);
    debug!("Generated spectator node_id_string: {}", node_id_string);

    let chitchat_id = ChitchatId::new(
      node_id_string.clone(),
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

    debug!(
      "Spectator chitchat config created: cluster_id={}, listen_addr={}, seed_nodes={:?}",
      config.cluster_id, config.listen_addr, config.seed_nodes
    );

    info!("Joining as spectator with node_id: {}", node_id_string);
    Self::join_with_config(config, vec![], None).await
  }

  async fn join_with_config(
    config: ChitchatConfig,
    key_values: Vec<(String, String)>,
    self_node: Option<Member>,
  ) -> Result<Self> {
    debug!(
      "Initializing chitchat with config and key_values: {:?}",
      key_values
    );

    let transport = UdpTransport;
    debug!("Created UDP transport");

    let chitchat_handle = spawn_chitchat(config, key_values, &transport).await?;
    info!(
      "Successfully spawned chitchat for node: {}",
      chitchat_handle
        .chitchat()
        .lock()
        .await
        .self_chitchat_id()
        .node_id
    );

    let chitchat = chitchat_handle.chitchat();
    debug!("Retrieved chitchat instance from handle");

    let cluster = Self {
      self_node: self_node.clone(),
      chitchat,
      _chitchat_handle: chitchat_handle,
      raft: Arc::new(RwLock::new(None)),
      raft_store: Arc::new(RwLock::new(None)),
    };

    if let Some(ref node) = self_node {
      info!(
        "Cluster join completed successfully for node: {} with service: {}",
        node.id, node.service
      );
    } else {
      info!("Spectator join completed successfully");
    }

    Ok(cluster)
  }

  pub async fn members(&self) -> Vec<Member> {
    debug!("Retrieving cluster members");
    let members = snapshot_members(self.chitchat.lock().await.state_snapshot())
      .into_iter()
      .unique_by(|m| m.service.clone())
      .collect::<Vec<_>>();
    debug!("Found {} unique members", members.len());
    members
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
    debug!("Setting service information: {}", service);

    let service_json = serde_json::to_string(&service)?;
    debug!("Serialized service to JSON: {}", service_json);

    self
      .chitchat
      .lock()
      .await
      .self_node_state()
      .set(SERVICE_KEY, &service_json);

    info!("Updated service information in chitchat: {}", service);
    Ok(())
  }

  /// Initialize OpenRAFT integration for this cluster
  pub async fn enable_raft(&self, node_id_str: String) -> Result<()> {
    info!("Enabling OpenRAFT integration for node: {}", node_id_str);

    if self.raft.read().await.is_some() {
      warn!("OpenRAFT already initialized for node: {}", node_id_str);
      return Ok(());
    }

    let node_id: NodeId = node_id_str.len() as u64;
    debug!(
      "Converted node_id_str '{}' to NodeId: {}",
      node_id_str, node_id
    );

    debug!("Creating OpenRAFT configuration");
    let raft_config = openraft::Config::default();
    let raft_config = Arc::new(raft_config.validate()?);

    debug!("Initializing OpenRAFT stores and network");
    let log_store = crate::distributed::log_store::LogStore::default();
    let state_machine_store = Arc::new(StateMachineStore::default());
    let network = ChitchatRaftNetwork::new();

    debug!("Creating Raft instance");
    let raft = Raft::new(
      node_id,
      raft_config,
      network,
      log_store,
      state_machine_store.clone(),
    )
    .await?;

    let members: BTreeMap<NodeId, BasicNode> =
      BTreeMap::from([(node_id, BasicNode::new(format!("127.0.0.1:8080")))]);
    debug!("Initializing Raft with members: {:?}", members);

    if let Err(e) = raft.initialize(members.clone()).await {
      match e {
        openraft::error::RaftError::APIError(e) => match e {
          InitializeError::NotAllowed(_) => {
            warn!("Raft cluster already initialized: {}", e);
          }
          InitializeError::NotInMembers(_) => {
            error!("Node not in members during initialization: {}", e);
            return Err(e.into());
          }
        },
        openraft::error::RaftError::Fatal(_) => {
          error!("Fatal error during Raft initialization: {}", e);
          return Err(e.into());
        }
      }
    }

    debug!("Storing initialized Raft components");
    let mut raft_guard = self.raft.write().await;
    *raft_guard = Some(raft);

    let mut store_guard = self.raft_store.write().await;
    *store_guard = Some(state_machine_store);

    info!(
      "OpenRAFT integration enabled successfully for node_id: {}",
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
    debug!("Processing Raft request: {:?}", request);

    if let Some(raft) = self.raft().await {
      debug!("Submitting request to OpenRAFT");
      match raft.client_write(request).await {
        Ok(response) => {
          debug!("Raft request completed successfully");
          Ok(response.data)
        }
        Err(e) => {
          error!("Raft request failed: {:?}", e);
          Err(anyhow::anyhow!("Raft request failed: {:?}", e))
        }
      }
    } else {
      error!("OpenRAFT not enabled - cannot process request");
      Err(anyhow::anyhow!("OpenRAFT not enabled"))
    }
  }

  pub async fn cluster_state(&self) -> ClusterStateSnapshot {
    debug!("Retrieving cluster state snapshot");
    self.chitchat.lock().await.state_snapshot()
  }

  pub async fn live_nodes(&self) -> Vec<ChitchatId> {
    let nodes = self
      .chitchat
      .lock()
      .await
      .live_nodes()
      .cloned()
      .collect::<Vec<_>>();
    debug!("Found {} live nodes", nodes.len());
    nodes
  }

  pub async fn dead_nodes(&self) -> Vec<ChitchatId> {
    let nodes = self
      .chitchat
      .lock()
      .await
      .dead_nodes()
      .cloned()
      .collect::<Vec<_>>();
    debug!("Found {} dead nodes", nodes.len());
    nodes
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

// Initialize file logging - call this early in your application
pub fn init_file_logging() -> Result<()> {
  use tracing_subscriber::{EnvFilter, fmt, prelude::*};

  // Get current working directory for debugging
  let current_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
  println!("Current working directory: {:?}", current_dir);

  // Create logs directory with full path
  let logs_dir = current_dir.join("logs");
  println!("Attempting to create logs directory at: {:?}", logs_dir);

  if let Err(e) = std::fs::create_dir_all(&logs_dir) {
    eprintln!(
      "Warning: Could not create logs directory at {:?}: {}",
      logs_dir, e
    );
    eprintln!("Falling back to console-only logging");
    return init_console_logging();
  }

  println!("Successfully created logs directory at: {:?}", logs_dir);

  // Verify the directory exists
  if !logs_dir.exists() {
    eprintln!("Error: Logs directory does not exist after creation attempt");
    return init_console_logging();
  }

  if !logs_dir.is_dir() {
    eprintln!("Error: Logs path exists but is not a directory");
    return init_console_logging();
  }

  println!("Logs directory verified successfully");

  let file_appender = tracing_appender::rolling::daily(&logs_dir, "chitchat_cluster.log");
  let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

  println!(
    "Created file appender for: {:?}/chitchat_cluster.log",
    logs_dir
  );

  tracing_subscriber::registry()
    .with(fmt::layer().with_writer(non_blocking))
    .with(fmt::layer().with_writer(std::io::stdout))
    .with(
      EnvFilter::from_default_env().add_directive("chitchat_openraft_axum_example=debug".parse()?),
    )
    .init();

  // Use println here since tracing might not be fully initialized yet
  println!(
    "File logging initialized - logs will be written to: {:?}/chitchat_cluster.log",
    logs_dir
  );

  // Store the guard to prevent it from being dropped
  std::mem::forget(_guard);

  Ok(())
}

// Simple console-only logging initialization
pub fn init_console_logging() -> Result<()> {
  use tracing_subscriber::{EnvFilter, fmt, prelude::*};

  tracing_subscriber::registry()
    .with(fmt::layer().with_writer(std::io::stdout))
    .with(
      EnvFilter::from_default_env().add_directive("chitchat_openraft_axum_example=debug".parse()?),
    )
    .init();

  info!("Console logging initialized with debug level");
  Ok(())
}
