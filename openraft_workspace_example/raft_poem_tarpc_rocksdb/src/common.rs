use std::{collections::BTreeMap, sync::Arc};

use openraft::Config;
use tokio::sync::RwLock;

use crate::{ExampleRaft, NodeId};

#[derive(Clone)]
pub struct Api {
  // pub num: Arc<Mutex<i64>>,
  pub id: NodeId,
  pub api_addr: String,
  pub rpc_addr: String,
  pub raft: ExampleRaft,
  pub key_values: Arc<RwLock<BTreeMap<String, String>>>,
  pub config: Arc<Config>,
}
