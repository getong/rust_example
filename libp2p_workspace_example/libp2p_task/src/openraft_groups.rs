use std::{
  collections::BTreeMap,
  error::Error,
  fmt,
  future::Future,
  path::{Path, PathBuf},
  sync::Arc,
};

use anyhow::Context;
use openraft::{
  BasicNode, RaftNetworkFactory, ServerState,
  async_runtime::WatchReceiver,
  network::{RPCOption, v2::RaftNetworkV2},
};

use crate::{
  raft_role::OpenRaftRoleTracker,
  raft_store::{self, RocksNodeId},
};

pub(crate) type TypeConfig = raft_store::TypeConfig;
pub(crate) type NodeId = <TypeConfig as openraft::RaftTypeConfig>::NodeId;
pub(crate) type GroupId = String;
pub(crate) type Raft = openraft::Raft<TypeConfig, raft_store::RocksStateMachine>;
pub(crate) type GroupHandleMap = BTreeMap<GroupId, GroupHandle>;

pub(crate) const USERS_GROUP: &str = "users";
pub(crate) const ORDERS_GROUP: &str = "orders";
pub(crate) const PRODUCTS_GROUP: &str = "products";

#[derive(Clone)]
pub(crate) struct GroupHandle {
  pub(crate) raft: Raft,
  pub(crate) db_dir: PathBuf,
}

#[derive(Clone)]
pub(crate) struct OpenraftGroups {
  local_node_id: NodeId,
  groups: Arc<GroupHandleMap>,
}

impl OpenraftGroups {
  pub(crate) fn local_node_id(&self) -> &NodeId {
    &self.local_node_id
  }

  pub(crate) fn groups(&self) -> &GroupHandleMap {
    &self.groups
  }

  pub(crate) fn default_group_id(&self) -> Option<&str> {
    if self.groups.contains_key(USERS_GROUP) {
      return Some(USERS_GROUP);
    }

    self.groups.keys().next().map(String::as_str)
  }
}

pub(crate) fn default_group_ids() -> Vec<GroupId> {
  vec![
    USERS_GROUP.to_string(),
    ORDERS_GROUP.to_string(),
    PRODUCTS_GROUP.to_string(),
  ]
}

pub(crate) fn parse_group_ids(value: &str) -> Vec<GroupId> {
  value
    .split(',')
    .map(str::trim)
    .filter(|group| !group.is_empty())
    .map(ToOwned::to_owned)
    .collect()
}

pub(crate) async fn start_openraft_groups(
  node_id: impl Into<String>,
  db_dir: &Path,
  group_ids: &[GroupId],
) -> anyhow::Result<OpenraftGroups> {
  if group_ids.is_empty() {
    anyhow::bail!("no openraft group ids configured");
  }

  let local_node_id = RocksNodeId::new(node_id.into());
  let config = Arc::new(
    openraft::Config::default()
      .validate()
      .context("validate openraft config")?,
  );
  let mut groups = BTreeMap::new();

  for group_id in group_ids {
    let group_db_dir = db_dir.join("openraft").join(group_id);
    let (log_store, state_machine) = raft_store::new::<TypeConfig, _>(&group_db_dir)
      .await
      .with_context(|| format!("open openraft store for group `{group_id}`"))?;
    let raft = Raft::new(
      local_node_id.clone(),
      config.clone(),
      NoopNetworkFactory::new(group_id.clone()),
      log_store,
      state_machine,
    )
    .await
    .with_context(|| format!("create openraft group `{group_id}`"))?;

    groups.insert(
      group_id.clone(),
      GroupHandle {
        raft,
        db_dir: group_db_dir,
      },
    );
  }

  Ok(OpenraftGroups {
    local_node_id,
    groups: Arc::new(groups),
  })
}

pub(crate) fn spawn_metrics_watcher(
  groups: &OpenraftGroups,
  default_group: impl Into<GroupId>,
  raft_role: OpenRaftRoleTracker,
) {
  let default_group = default_group.into();
  let Some(default_handle) = groups.groups.get(&default_group).cloned() else {
    tracing::warn!(group = %default_group, "openraft metrics watcher skipped; group not found");
    return;
  };

  tokio::spawn(async move {
    let mut metrics_rx = default_handle.raft.metrics();
    loop {
      let metrics = metrics_rx.borrow_watched().clone();
      raft_role.set_state(Some(metrics.state)).await;
      raft_role
        .set_current_leader(metrics.current_leader.map(|leader| leader.to_string()))
        .await;

      if metrics_rx.changed().await.is_err() {
        return;
      }
    }
  });
}

pub(crate) fn metrics_summary(groups: &OpenraftGroups) -> Vec<String> {
  groups
    .groups
    .iter()
    .map(|(group_id, handle)| {
      let metrics = handle.raft.metrics().borrow_watched().clone();
      format!(
        "group={group_id} state={:?} leader={} db={}",
        metrics.state,
        metrics
          .current_leader
          .map_or_else(|| "<unknown>".to_string(), |leader| leader.to_string()),
        handle.db_dir.display()
      )
    })
    .collect()
}

#[derive(Clone)]
struct NoopNetworkFactory {
  group_id: GroupId,
}

impl NoopNetworkFactory {
  fn new(group_id: GroupId) -> Self {
    Self { group_id }
  }
}

impl RaftNetworkFactory<TypeConfig> for NoopNetworkFactory {
  type Network = NoopNetwork;

  async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
    NoopNetwork {
      group_id: self.group_id.clone(),
      target,
      target_addr: node.addr.clone(),
    }
  }
}

struct NoopNetwork {
  group_id: GroupId,
  target: NodeId,
  target_addr: String,
}

impl NoopNetwork {
  fn unreachable(&self, op: &str) -> openraft::error::Unreachable<TypeConfig> {
    openraft::error::Unreachable::new(&NoopNetworkError {
      group_id: self.group_id.clone(),
      target: self.target.clone(),
      target_addr: self.target_addr.clone(),
      op: op.to_string(),
    })
  }
}

impl RaftNetworkV2<TypeConfig> for NoopNetwork {
  async fn append_entries(
    &mut self,
    _rpc: openraft::raft::AppendEntriesRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<
    openraft::raft::AppendEntriesResponse<TypeConfig>,
    openraft::error::RPCError<TypeConfig>,
  > {
    Err(openraft::error::RPCError::Unreachable(
      self.unreachable("append_entries"),
    ))
  }

  async fn vote(
    &mut self,
    _rpc: openraft::raft::VoteRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<openraft::raft::VoteResponse<TypeConfig>, openraft::error::RPCError<TypeConfig>> {
    Err(openraft::error::RPCError::Unreachable(
      self.unreachable("vote"),
    ))
  }

  async fn full_snapshot(
    &mut self,
    _vote: <TypeConfig as openraft::RaftTypeConfig>::Vote,
    _snapshot: openraft::alias::SnapshotOf<TypeConfig>,
    _cancel: impl Future<Output = openraft::error::ReplicationClosed> + openraft::OptionalSend + 'static,
    _option: RPCOption,
  ) -> Result<
    openraft::raft::SnapshotResponse<TypeConfig>,
    openraft::error::StreamingError<TypeConfig>,
  > {
    Err(openraft::error::StreamingError::Unreachable(
      self.unreachable("full_snapshot"),
    ))
  }
}

#[derive(Debug)]
struct NoopNetworkError {
  group_id: GroupId,
  target: NodeId,
  target_addr: String,
  op: String,
}

impl fmt::Display for NoopNetworkError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "openraft group={} has no libp2p raft rpc transport for {} to target={} addr={}",
      self.group_id, self.op, self.target, self.target_addr
    )
  }
}

impl Error for NoopNetworkError {}

pub(crate) fn server_state_name(state: ServerState) -> &'static str {
  match state {
    ServerState::Learner => "learner",
    ServerState::Follower => "follower",
    ServerState::Candidate => "candidate",
    ServerState::Leader => "leader",
    ServerState::Shutdown => "shutdown",
  }
}
