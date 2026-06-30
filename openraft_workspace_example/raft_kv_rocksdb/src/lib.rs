#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::{path::Path, sync::Arc};

use axum::{
  Router,
  routing::{get, post},
};
use openraft::Config;
use tokio::net::TcpListener;

use crate::{
  app::App,
  network::{api, management, raft},
  store::new_storage,
};

pub mod app;
pub mod network;
pub mod store;

pub type NodeId = u64;

openraft::declare_raft_types!(
    pub TypeConfig:
        D = types_kv::Request,
        R = types_kv::Response,
        Node = openraft::NodeInfo,
        SnapshotData = std::io::Cursor<Vec<u8>>,
);

pub type LogStore = openraft_rocksstore::log_store::RocksLogStore<TypeConfig>;
pub type StateMachineStore = store::StateMachineStore;
pub type Raft = openraft::Raft<TypeConfig, store::StateMachineStore>;

pub mod typ;

pub async fn start_example_raft_node<P>(
  node_id: NodeId,
  dir: P,
  addr: String,
) -> std::io::Result<()>
where
  P: AsRef<Path>,
{
  // Create a configuration for the raft instance.
  let config = Config {
    heartbeat_interval: 250,
    election_timeout_min: 299,
    ..Default::default()
  };

  let config = Arc::new(config.validate().unwrap());

  let (log_store, state_machine_store) = new_storage(&dir).await;

  let kvs = state_machine_store.data.kvs.clone();

  let network = network_v2_http::NetworkFactory::new();

  // Create a local raft instance.
  let raft = openraft::Raft::new(
    node_id,
    config.clone(),
    network,
    log_store,
    state_machine_store,
  )
  .await
  .unwrap();

  let app_data = Arc::new(App {
    id: node_id,
    addr: addr.clone(),
    raft,
    key_values: kvs,
    config,
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
    // application API
    .route("/write", post(api::write))
    .route("/read", post(api::read))
    .route("/linearizable_read", post(api::linearizable_read))
    .with_state(app_data);

  let listener = TcpListener::bind(&addr).await?;
  axum::serve(listener, router).await
}
