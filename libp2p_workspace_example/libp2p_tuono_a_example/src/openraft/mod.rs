#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::{
  fmt::Display,
  path::Path,
  sync::{Arc, LazyLock},
};

use openraft::Config;
use tokio::{net::TcpListener, sync::Mutex, task};

use crate::openraft::{
  app::App,
  network::Network,
  store::{Request, Response, new_storage},
};

pub mod app;
pub mod client;
pub mod network;
pub mod rocksdb;
pub mod store;
pub mod typ;

pub type NodeId = u64;

pub static LAZY_RAFT: LazyLock<Arc<Mutex<Option<openraft::Raft<TypeConfig>>>>> =
  LazyLock::new(|| Arc::new(Mutex::new(None)));

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
pub struct Node {
  pub rpc_addr: String,
  pub api_addr: String,
}

impl Display for Node {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Node {{ rpc_addr: {}, api_addr: {} }}",
      self.rpc_addr, self.api_addr
    )
  }
}

openraft::declare_raft_types!(
    pub TypeConfig:
        D = Request,
        R = Response,
        Node = Node,
);

pub async fn start_example_raft_node<P>(node_id: NodeId, dir: P) -> std::io::Result<()>
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

  // Create the network layer that will connect and communicate the raft instances and
  // will be used in conjunction with the store created above.
  let network = Network {};

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
  {
    let mut raft_lock = LAZY_RAFT.lock().await;
    *raft_lock = Some(raft);
  }

  Ok(())
}

// async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
//   tokio::spawn(fut);
// }
