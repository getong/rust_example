use std::sync::Arc;

use crate::ExampleRaft;
use crate::NodeId;
use openraft::Config;
use std::collections::BTreeMap;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Api {
  // pub num: Arc<Mutex<i64>>,
  pub id: NodeId,
  pub api_addr: String,
  pub rcp_addr: String,
  pub raft: ExampleRaft,
  pub key_values: Arc<RwLock<BTreeMap<String, String>>>,
  pub config: Arc<Config>,
}
