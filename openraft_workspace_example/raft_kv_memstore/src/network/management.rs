use std::{
  collections::{BTreeMap, BTreeSet},
  sync::Arc,
};

use axum::{Json, extract::State, response::IntoResponse};
use openraft::{
  NodeInfo, RaftMetrics, ReadPolicy,
  alias::LogIdOf,
  async_runtime::WatchReceiver,
  error::{Infallible, decompose::DecomposeResult},
};
use serde::{Deserialize, Serialize};

use crate::{NodeId, TypeConfig, app::App};

/// Serializable representation of linearizer data for follower reads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearizerData {
  pub node_id: NodeId,
  pub read_log_id: LogIdOf<TypeConfig>,
  pub applied: Option<LogIdOf<TypeConfig>>,
}

// --- Cluster management

/// Add a node as **Learner**.
///
/// A Learner receives log replication from the leader but does not vote.
/// This should be done before adding a node as a member into the cluster
/// (by calling `change-membership`)
pub async fn add_learner(
  State(app): State<Arc<App>>,
  req: Json<(NodeId, String)>,
) -> impl IntoResponse {
  let (node_id, addr) = req.0.clone();
  let node = NodeInfo::new(addr.clone(), addr);
  let res = app
    .raft
    .add_learner(node_id, node, true)
    .await
    .decompose()
    .unwrap();
  Json(res)
}

/// Changes specified learners to members, or remove members.
pub async fn change_membership(
  State(app): State<Arc<App>>,
  req: Json<BTreeSet<NodeId>>,
) -> impl IntoResponse {
  let res = app
    .raft
    .change_membership(req.0, false)
    .await
    .decompose()
    .unwrap();
  Json(res)
}

/// Initialize a single-node cluster if the `req` is empty vec.
/// Otherwise initialize a cluster with the `req` specified vec of node-id and node-address
pub async fn init(
  State(app): State<Arc<App>>,
  req: Json<Vec<(NodeId, String)>>,
) -> impl IntoResponse {
  let mut nodes = BTreeMap::new();
  if req.0.is_empty() {
    nodes.insert(app.id, NodeInfo::new(app.addr.clone(), app.addr.clone()));
  } else {
    for (id, addr) in req.0.into_iter() {
      nodes.insert(id, NodeInfo::new(addr.clone(), addr));
    }
  };
  let res = app.raft.initialize(nodes).await.decompose().unwrap();
  Json(res)
}

/// Get the latest metrics of the cluster
pub async fn metrics(State(app): State<Arc<App>>) -> impl IntoResponse {
  let metrics = app.raft.metrics().borrow_watched().clone();

  let res: Result<RaftMetrics<TypeConfig>, Infallible> = Ok(metrics);
  Json(res)
}

/// Get linearizer data for performing linearizable reads on followers
///
/// This endpoint is used by followers to obtain linearizer data from the leader.
/// The follower can then reconstruct a Linearizer and wait for its local state
/// machine to catch up before performing a linearizable read.
pub async fn get_linearizer(State(app): State<Arc<App>>) -> impl IntoResponse {
  let linearizer = app
    .raft
    .get_read_linearizer(ReadPolicy::ReadIndex)
    .await
    .decompose()
    .unwrap();

  let data = match linearizer {
    Ok(lin) => {
      let data = LinearizerData {
        node_id: *lin.node_id(),
        read_log_id: *lin.read_log_id(),
        applied: lin.applied().cloned(),
      };
      Ok(data)
    }
    Err(e) => Err(e),
  };

  Json(data)
}
