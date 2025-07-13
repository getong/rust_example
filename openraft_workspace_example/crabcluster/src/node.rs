use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use axum::{
  Router,
  routing::{get, post},
};
use openraft::{BasicNode, Config, Raft};
use uuid::Uuid;

use crate::{
  network::{
    management::{add_learner, change_membership, get_id, init, metrics},
    raft::{append, snapshot, vote},
    user_api::{kv_read, kv_write},
  },
  raft_network::RaftNetworkClient,
  store::{RaftRequest, RaftResponse, RaftStore},
};

pub type NodeId = Uuid;

openraft::declare_raft_types!(
    /// Declare the type configuration for K/V store.
    pub RaftTypeConfig: D = RaftRequest, R = RaftResponse, NodeId = NodeId, Node = BasicNode
);

pub type RaftConfig = Raft<RaftTypeConfig>;

// Representation of an application state. This struct can be shared around to share
// instances of raft, store and more.
#[derive(Clone)]
pub struct RaftApp {
  pub id: NodeId,
  pub bind_addr: SocketAddr,
  pub raft: RaftConfig,
  pub store: Arc<RaftStore>,
  pub config: Arc<Config>,
}

impl RaftApp {
  /// Get the cluster configuration
  pub fn get_config(&self) -> &Config {
    &self.config
  }
}

pub async fn start_node(node_id: NodeId, bind_addr: SocketAddr) -> Result<()> {
  // Create a configuration for the raft instance optimized for multi-node clusters
  let config = Arc::new(
    Config {
      cluster_name: "crabcluster".to_string(),
      heartbeat_interval: 100,   // Faster heartbeat for better responsiveness
      election_timeout_min: 200, // Faster elections for quicker recovery
      election_timeout_max: 400, // Maximum election timeout
      max_payload_entries: 500,  // Larger payload for better throughput
      replication_lag_threshold: 2000, // Allow more lag before snapshot
      purge_batch_size: 512,     // Larger batch size for log purging
      ..Default::default()
    }
    .validate()
    .unwrap(),
  );

  // Create a instance of where the Raft data will be stored.
  let store = Arc::new(RaftStore::default());

  // Create the network layer that will connect and communicate the raft instances and
  // will be used in conjunction with the store created above.
  let network = RaftNetworkClient {};

  // Create a local raft instance.
  let raft = Raft::new(
    node_id,
    config.clone(),
    network,
    store.clone(),
    store.clone(),
  )
  .await
  .unwrap();

  // Create an application that will store all the instances created above
  let app_state = RaftApp {
    id: node_id,
    bind_addr,
    raft,
    store,
    config,
  };

  let app = Router::new()
    .route("/init", get(init))
    .route("/raft-append", post(append))
    .route("/raft-snapshot", post(snapshot))
    .route("/raft-vote", post(vote))
    .route("/get-id", get(get_id))
    .route("/metrics", get(metrics))
    .route("/config", get(crate::network::management::get_config))
    .route("/add-learner", post(add_learner))
    .route("/change-membership", post(change_membership))
    .route("/read", post(kv_read))
    .route("/write", post(kv_write))
    .with_state(app_state)
    .layer(tower_http::trace::TraceLayer::new_for_http());
  axum::serve(
    tokio::net::TcpListener::bind(&bind_addr).await?,
    app.into_make_service(),
  )
  .await
  .unwrap();

  Ok(())
}
