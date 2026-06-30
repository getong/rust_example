use std::{
  collections::{BTreeMap, BTreeSet},
  sync::Arc,
};

use axum::{Json, extract::State, response::IntoResponse};
use openraft::{
  NodeInfo,
  async_runtime::WatchReceiver,
  error::{Infallible, decompose::DecomposeResult},
};

use crate::{NodeId, app::App, typ::*};

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
  let (node_id, api_addr) = req.0;
  let node = NodeInfo::new(api_addr, "");
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
  let body = req.0;
  let res = app
    .raft
    .change_membership(body, false)
    .await
    .decompose()
    .unwrap();
  Json(res)
}

/// Initialize a cluster.
pub async fn init(
  State(app): State<Arc<App>>,
  req: Json<Vec<(NodeId, String)>>,
) -> impl IntoResponse {
  let mut nodes = BTreeMap::new();
  if req.0.is_empty() {
    nodes.insert(app.id, NodeInfo::new(app.addr.clone(), ""));
  } else {
    for (id, addr) in req.0.into_iter() {
      nodes.insert(id, NodeInfo::new(addr, ""));
    }
  };
  let res = app.raft.initialize(nodes).await.decompose().unwrap();
  Json(res)
}

/// Get the latest metrics of the cluster
pub async fn metrics(State(app): State<Arc<App>>) -> impl IntoResponse {
  let metrics = app.raft.metrics().borrow_watched().clone();

  let res: Result<RaftMetrics, Infallible> = Ok(metrics);
  Json(res)
}
