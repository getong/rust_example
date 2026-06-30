#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::sync::Arc;

use axum::{
  Router,
  routing::{get, post},
};
use openraft::Config;
use tokio::net::TcpListener;

use crate::{
  app::App,
  network::{api, management, raft},
};

pub mod app;
pub mod network;
pub mod store;
#[cfg(test)]
mod test;

pub type NodeId = u64;

openraft::declare_raft_types!(
    /// Declare the type configuration for example K/V store.
    pub TypeConfig:
        D = types_kv::Request,
        R = types_kv::Response,
        Node = openraft::NodeInfo,
        SnapshotData = std::io::Cursor<Vec<u8>>,
);

pub type LogStore = store::LogStore<TypeConfig>;
pub type StateMachineStore = store::StateMachineStore<TypeConfig>;
pub type Raft = openraft::Raft<TypeConfig, store::StateMachineStore<TypeConfig>>;

pub mod typ;

pub async fn start_example_raft_node(node_id: NodeId, http_addr: String) -> std::io::Result<()> {
  // Create a configuration for the raft instance.
  let config = Config {
    heartbeat_interval: 500,
    election_timeout_min: 1500,
    election_timeout_max: 3000,
    ..Default::default()
  };

  let config = Arc::new(config.validate().unwrap());

  // Create a instance of where the Raft logs will be stored.
  let log_store = LogStore::default();
  // Create a instance of where the Raft data will be stored.
  let state_machine_store = StateMachineStore::default();

  // Create the network layer that will connect and communicate the raft instances and
  // will be used in conjunction with the store created above.
  let network = network_v2_http::NetworkFactory::new();

  // Create a local raft instance.
  let raft = openraft::Raft::new(
    node_id,
    config.clone(),
    network,
    log_store.clone(),
    state_machine_store.clone(),
  )
  .await
  .unwrap();

  let app_data = Arc::new(App {
    id: node_id,
    addr: http_addr.clone(),
    raft,
    state_machine_store,
  });

  let router = Router::new()
    // raft internal RPC
    .route("/append", post(raft::append))
    .route("/snapshot", post(raft::snapshot))
    .route("/transfer-leader", post(raft::transfer_leader))
    .route("/vote", post(raft::vote))
    // admin API
    .route("/init", post(management::init))
    .route("/add-learner", post(management::add_learner))
    .route("/change-membership", post(management::change_membership))
    .route("/metrics", get(management::metrics))
    .route("/get_linearizer", post(management::get_linearizer))
    // application API
    .route("/write", post(api::write))
    .route("/read", post(api::read))
    .route("/linearizable_read", post(api::linearizable_read))
    .route("/follower_read", post(api::follower_read))
    .with_state(app_data);

  let listener = TcpListener::bind(&http_addr).await?;
  axum::serve(listener, router).await
}
