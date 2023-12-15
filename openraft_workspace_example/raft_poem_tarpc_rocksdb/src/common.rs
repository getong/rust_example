use std::sync::Arc;
use tokio::sync::Mutex;

use openraft::Config;

use crate::ExampleRaft;
use crate::NodeId;
use crate::Store;

#[derive(Clone)]
pub struct Api {
  pub num: Arc<Mutex<i64>>,
  pub id: NodeId,
  pub api_addr: String,
  pub rcp_addr: String,
  pub raft: ExampleRaft,
  pub store: Arc<Store>,
  pub config: Arc<Config>,
}
