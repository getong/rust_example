use std::{
  collections::{BTreeMap, BTreeSet},
  fmt,
  io::{self, Cursor},
  net::SocketAddr,
  sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
  },
};

use axum::{
  Json, Router,
  extract::State,
  http::StatusCode,
  response::{IntoResponse, Response},
  routing::{get, post},
};
use futures::{Stream, TryStreamExt, lock::Mutex};
use kameo::{
  Actor,
  actor::{ActorRef, Spawn},
  error::Infallible,
  message::{Context, Message},
};
use openraft::{
  Config, EntryPayload, NodeInfo, OptionalSend, RaftSnapshotBuilder, ReadPolicy, Snapshot,
  alias::{LogIdOf, SnapshotMetaOf, SnapshotOf, StoredMembershipOf, VoteOf},
  async_runtime::WatchReceiver,
  errors::{ClientWriteError, ForwardToLeader, RaftError},
  raft::{
    AppendEntriesRequest, SnapshotResponse, TransferLeaderRequest, TransferLeaderResponse,
    VoteRequest,
  },
  storage::{EntryResponder, RaftStateMachine},
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

pub type NodeId = u64;
pub type LogStore = mem_log::LogStore<TypeConfig>;
pub type Raft = openraft::Raft<TypeConfig, RaftStateMachineStore>;
type HttpWriteResult = Result<
  openraft::raft::ClientWriteResponse<TypeConfig>,
  RaftError<TypeConfig, ClientWriteError<TypeConfig>>,
>;

/// Command replicated through Raft before being applied to the local actor.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SetCommand {
  pub key: String,
  pub value: String,
}

impl SetCommand {
  pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
    Self {
      key: key.into(),
      value: value.into(),
    }
  }
}

impl fmt::Display for SetCommand {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "SetCommand {{ key: {}, value: {} }}",
      self.key, self.value
    )
  }
}

/// Data state owned by the Kameo actor.
#[derive(Default, Debug)]
pub struct KvStoreActor {
  state: BTreeMap<String, String>,
}

impl Actor for KvStoreActor {
  type Args = Self;
  type Error = Infallible;

  async fn on_start(state: Self::Args, _actor_ref: ActorRef<Self>) -> Result<Self, Self::Error> {
    Ok(state)
  }
}

impl Message<SetCommand> for KvStoreActor {
  type Reply = Option<String>;

  async fn handle(
    &mut self,
    msg: SetCommand,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.state.insert(msg.key, msg.value)
  }
}

#[derive(Clone, Debug)]
struct DumpState;

impl Message<DumpState> for KvStoreActor {
  type Reply = BTreeMap<String, String>;

  async fn handle(
    &mut self,
    _msg: DumpState,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.state.clone()
  }
}

#[derive(Clone, Debug)]
struct InstallState(BTreeMap<String, String>);

impl Message<InstallState> for KvStoreActor {
  type Reply = ();

  async fn handle(
    &mut self,
    msg: InstallState,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    self.state = msg.0;
  }
}

openraft::declare_raft_types!(
    /// Type configuration for the Kameo-backed KV state machine.
    pub TypeConfig:
        D = SetCommand,
        R = Option<String>,
        Node = NodeInfo,
        SnapshotData = Cursor<Vec<u8>>,
);

pub type KameoRaft<SM = RaftStateMachineStore> = openraft::Raft<TypeConfig, SM>;

#[derive(Clone)]
pub struct AppState {
  pub id: NodeId,
  pub addr: String,
  pub raft: Raft,
  pub state_machine_store: RaftStateMachineStore,
  pub http_client: reqwest::Client,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct LeaderInfo {
  pub local_node_id: NodeId,
  pub local_addr: String,
  pub leader_id: Option<NodeId>,
  pub leader_addr: Option<String>,
  pub is_leader: bool,
}

#[derive(Debug)]
pub struct StoredSnapshot {
  pub meta: SnapshotMetaOf<TypeConfig>,
  pub data: Vec<u8>,
}

#[derive(Debug)]
struct RaftStateMachineInner {
  last_applied_log: Option<LogIdOf<TypeConfig>>,
  last_membership: StoredMembershipOf<TypeConfig>,
  snapshot_idx: AtomicU64,
  current_snapshot: Option<StoredSnapshot>,
}

impl Default for RaftStateMachineInner {
  fn default() -> Self {
    Self {
      last_applied_log: None,
      last_membership: StoredMembershipOf::<TypeConfig>::default(),
      snapshot_idx: AtomicU64::new(0),
      current_snapshot: None,
    }
  }
}

impl RaftStateMachineInner {
  fn next_snapshot_idx(&self) -> u64 {
    self.snapshot_idx.fetch_add(1, Ordering::Relaxed) + 1
  }
}

/// OpenRaft state machine wrapper that delegates application commands to a local Kameo actor.
#[derive(Clone, Debug)]
pub struct RaftStateMachineStore {
  actor_ref: ActorRef<KvStoreActor>,
  inner: Arc<Mutex<RaftStateMachineInner>>,
  operation_lock: Arc<Mutex<()>>,
}

impl RaftStateMachineStore {
  pub fn new(actor_ref: ActorRef<KvStoreActor>) -> Self {
    Self {
      actor_ref,
      inner: Arc::new(Mutex::new(RaftStateMachineInner::default())),
      operation_lock: Arc::new(Mutex::new(())),
    }
  }

  pub fn spawn_actor() -> Self {
    Self::new(KvStoreActor::spawn_default())
  }

  pub fn actor_ref(&self) -> &ActorRef<KvStoreActor> {
    &self.actor_ref
  }

  pub async fn dump_state(&self) -> Result<BTreeMap<String, String>, io::Error> {
    self
      .actor_ref
      .ask(DumpState)
      .send()
      .await
      .map_err(actor_send_io_error)
  }
}

impl RaftSnapshotBuilder<TypeConfig> for RaftStateMachineStore {
  async fn build_snapshot(&mut self) -> Result<SnapshotOf<TypeConfig>, io::Error> {
    let _operation_guard = self.operation_lock.lock().await;
    let state = self.dump_state().await?;
    let data =
      serde_json::to_vec(&state).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let mut inner = self.inner.lock().await;
    let snapshot_idx = inner.next_snapshot_idx();
    let snapshot_id = match &inner.last_applied_log {
      Some(last) => format!(
        "{}-{}-{}",
        last.committed_leader_id(),
        last.index(),
        snapshot_idx
      ),
      None => format!("--{}", snapshot_idx),
    };

    let meta = SnapshotMetaOf::<TypeConfig> {
      last_log_id: inner.last_applied_log.clone(),
      last_membership: inner.last_membership.clone(),
      snapshot_id,
    };

    inner.current_snapshot = Some(StoredSnapshot {
      meta: meta.clone(),
      data: data.clone(),
    });

    Ok(SnapshotOf::<TypeConfig> {
      meta,
      snapshot: Cursor::new(data),
    })
  }
}

impl RaftStateMachine<TypeConfig> for RaftStateMachineStore {
  type SnapshotBuilder = Self;

  async fn applied_state(
    &mut self,
  ) -> Result<(Option<LogIdOf<TypeConfig>>, StoredMembershipOf<TypeConfig>), io::Error> {
    let inner = self.inner.lock().await;
    Ok((
      inner.last_applied_log.clone(),
      inner.last_membership.clone(),
    ))
  }

  async fn apply<Strm>(&mut self, mut entries: Strm) -> Result<(), io::Error>
  where
    Strm: Stream<Item = Result<EntryResponder<TypeConfig>, io::Error>> + Unpin + OptionalSend,
  {
    while let Some((entry, responder)) = entries.try_next().await? {
      let _operation_guard = self.operation_lock.lock().await;
      let response = match entry.payload {
        EntryPayload::Blank => None,
        EntryPayload::Normal(cmd) => self
          .actor_ref
          .ask(cmd)
          .send()
          .await
          .map_err(actor_send_io_error)?,
        EntryPayload::Membership(membership) => {
          let mut inner = self.inner.lock().await;
          inner.last_membership =
            StoredMembershipOf::<TypeConfig>::new(Some(entry.log_id.clone()), membership);
          None
        }
      };

      let mut inner = self.inner.lock().await;
      inner.last_applied_log = Some(entry.log_id);
      drop(inner);

      if let Some(responder) = responder {
        responder.send(response);
      }
    }

    Ok(())
  }

  async fn begin_receiving_snapshot(&mut self) -> Result<Cursor<Vec<u8>>, io::Error> {
    Ok(Cursor::new(Vec::new()))
  }

  async fn install_snapshot(
    &mut self,
    meta: &SnapshotMetaOf<TypeConfig>,
    snapshot_data: Cursor<Vec<u8>>,
  ) -> Result<(), io::Error> {
    let _operation_guard = self.operation_lock.lock().await;
    let data = snapshot_data.into_inner();
    let state: BTreeMap<String, String> =
      serde_json::from_slice(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    self
      .actor_ref
      .ask(InstallState(state))
      .send()
      .await
      .map_err(actor_send_io_error)?;

    let mut inner = self.inner.lock().await;
    inner.last_applied_log = meta.last_log_id.clone();
    inner.last_membership = meta.last_membership.clone();
    inner.current_snapshot = Some(StoredSnapshot {
      meta: meta.clone(),
      data,
    });

    Ok(())
  }

  async fn get_current_snapshot(&mut self) -> Result<Option<SnapshotOf<TypeConfig>>, io::Error> {
    let inner = self.inner.lock().await;
    Ok(
      inner
        .current_snapshot
        .as_ref()
        .map(|stored_snapshot| SnapshotOf::<TypeConfig> {
          meta: stored_snapshot.meta.clone(),
          snapshot: Cursor::new(stored_snapshot.data.clone()),
        }),
    )
  }

  async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
    self.clone()
  }
}

pub async fn handle_client_write<SM>(
  raft: Arc<openraft::Raft<TypeConfig, SM>>,
  key: impl Into<String>,
  value: impl Into<String>,
) -> Result<Option<String>, String>
where
  SM: RaftStateMachine<TypeConfig>,
{
  let command = SetCommand::new(key, value);

  match raft.client_write(command).await {
    Ok(response) => Ok(response.data),
    Err(RaftError::APIError(ClientWriteError::ForwardToLeader(err))) => Err(format!(
      "not leader; forward request to leader {:?}",
      err.leader_id
    )),
    Err(err) => Err(format!("raft write error: {err:?}")),
  }
}

fn actor_send_io_error<M, E>(err: kameo::error::SendError<M, E>) -> io::Error
where
  E: fmt::Debug,
{
  io::Error::new(io::ErrorKind::BrokenPipe, format!("{err:?}"))
}

fn current_leader_info(app: &AppState) -> LeaderInfo {
  let raft_metrics = app.raft.metrics().borrow_watched().clone();
  let leader_id = raft_metrics.current_leader;
  let leader_addr = leader_id.and_then(|id| {
    raft_metrics
      .membership_config
      .get_node(&id)
      .map(|node| node.raft_addr.clone())
  });

  LeaderInfo {
    local_node_id: app.id,
    local_addr: app.addr.clone(),
    leader_id,
    leader_addr,
    is_leader: leader_id == Some(app.id),
  }
}

fn leader_addr_from_forward(
  app: &AppState,
  forward: &ForwardToLeader<TypeConfig>,
) -> Option<String> {
  if let Some(node) = &forward.leader_node {
    return Some(node.raft_addr.clone());
  }

  let raft_metrics = app.raft.metrics().borrow_watched().clone();
  forward.leader_id.and_then(|id| {
    raft_metrics
      .membership_config
      .get_node(&id)
      .map(|node| node.raft_addr.clone())
  })
}

async fn forward_write_to_leader(
  app: &AppState,
  leader_addr: &str,
  command: &SetCommand,
) -> Result<HttpWriteResult, String> {
  let url = format!("http://{leader_addr}/write-local");
  let response = app
    .http_client
    .post(&url)
    .json(command)
    .send()
    .await
    .map_err(|e| format!("failed to forward write to leader {leader_addr}: {e}"))?;

  let status = response.status();
  if !status.is_success() {
    return Err(format!(
      "leader {leader_addr} rejected forwarded write with HTTP {status}"
    ));
  }

  response
    .json::<HttpWriteResult>()
    .await
    .map_err(|e| format!("failed to decode leader write response from {leader_addr}: {e}"))
}

pub async fn start_kameo_raft_node(node_id: NodeId, http_addr: String) -> io::Result<()> {
  let config = Config {
    heartbeat_interval: 500,
    election_timeout_min: 1500,
    election_timeout_max: 3000,
    ..Default::default()
  };
  let config = Arc::new(
    config
      .validate()
      .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("{e:?}")))?,
  );

  let log_store = LogStore::default();
  let state_machine_store = RaftStateMachineStore::spawn_actor();
  let network = network_v2_http::NetworkFactory::new();
  let http_client = reqwest::Client::builder()
    .no_proxy()
    .build()
    .map_err(|e| io::Error::other(format!("{e:?}")))?;

  let raft = openraft::Raft::new(
    node_id,
    config,
    network,
    log_store,
    state_machine_store.clone(),
  )
  .await
  .map_err(|e| io::Error::other(format!("{e:?}")))?;

  let app_data = Arc::new(AppState {
    id: node_id,
    addr: http_addr.clone(),
    raft,
    state_machine_store,
    http_client,
  });

  let router = Router::new()
    .route("/vote", post(vote))
    .route("/append", post(append))
    .route("/snapshot", post(snapshot_rpc))
    .route("/transfer-leader", post(transfer_leader))
    .route("/init", post(init))
    .route("/add-learner", post(add_learner))
    .route("/change-membership", post(change_membership))
    .route("/metrics", get(metrics))
    .route("/leader", get(leader))
    .route("/write", post(write))
    .route("/write-local", post(write_local))
    .route("/read", post(read))
    .route("/linearizable-read", post(linearizable_read))
    .with_state(app_data);

  let addr: SocketAddr = http_addr
    .parse()
    .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("{e:?}")))?;
  let listener = TcpListener::bind(addr).await?;
  axum::serve(listener, router).await
}

async fn vote(
  State(app): State<Arc<AppState>>,
  req: Json<VoteRequest<TypeConfig>>,
) -> impl IntoResponse {
  let res = app.raft.vote(req.0).await;
  Json(res)
}

async fn append(
  State(app): State<Arc<AppState>>,
  req: Json<AppendEntriesRequest<TypeConfig>>,
) -> impl IntoResponse {
  let res = app.raft.append_entries(req.0).await;
  Json(res)
}

async fn snapshot_rpc(
  State(app): State<Arc<AppState>>,
  req: Json<(VoteOf<TypeConfig>, SnapshotMetaOf<TypeConfig>, Vec<u8>)>,
) -> impl IntoResponse {
  let (vote, meta, data) = req.0;
  let snapshot = Snapshot {
    meta,
    snapshot: Cursor::new(data),
  };
  let res: Result<SnapshotResponse<TypeConfig>, RaftError<TypeConfig>> = app
    .raft
    .install_full_snapshot(vote, snapshot)
    .await
    .map_err(RaftError::Fatal);
  Json(res)
}

async fn transfer_leader(
  State(app): State<Arc<AppState>>,
  req: Json<TransferLeaderRequest<TypeConfig>>,
) -> impl IntoResponse {
  let res: Result<TransferLeaderResponse<TypeConfig>, RaftError<TypeConfig>> = app
    .raft
    .handle_transfer_leader(req.0)
    .await
    .map_err(RaftError::Fatal);
  Json(res)
}

async fn init(
  State(app): State<Arc<AppState>>,
  req: Json<Vec<(NodeId, String)>>,
) -> impl IntoResponse {
  let mut nodes = BTreeMap::new();
  if req.is_empty() {
    nodes.insert(app.id, NodeInfo::new(app.addr.clone(), app.addr.clone()));
  } else {
    for (id, addr) in req.0 {
      nodes.insert(id, NodeInfo::new(addr.clone(), addr));
    }
  }

  let res = app.raft.initialize(nodes).await;
  Json(res)
}

async fn add_learner(
  State(app): State<Arc<AppState>>,
  req: Json<(NodeId, String)>,
) -> impl IntoResponse {
  let (node_id, addr) = req.0;
  let res = app
    .raft
    .add_learner(node_id, NodeInfo::new(addr.clone(), addr), true)
    .await;
  Json(res)
}

async fn change_membership(
  State(app): State<Arc<AppState>>,
  req: Json<BTreeSet<NodeId>>,
) -> impl IntoResponse {
  let res = app.raft.change_membership(req.0, false).await;
  Json(res)
}

async fn metrics(State(app): State<Arc<AppState>>) -> impl IntoResponse {
  Json(app.raft.metrics().borrow_watched().clone())
}

async fn leader(State(app): State<Arc<AppState>>) -> impl IntoResponse {
  Json(current_leader_info(&app))
}

async fn write(State(app): State<Arc<AppState>>, req: Json<SetCommand>) -> Response {
  let command = req.0;
  let local_res = app.raft.client_write(command.clone()).await;

  let Err(err) = local_res else {
    return Json(local_res).into_response();
  };

  let Some(forward) = err.forward_to_leader() else {
    let res: HttpWriteResult = Err(err);
    return Json(res).into_response();
  };

  let Some(leader_addr) = leader_addr_from_forward(&app, forward) else {
    let res: HttpWriteResult = Err(err);
    return Json(res).into_response();
  };

  if leader_addr == app.addr {
    let res: HttpWriteResult = Err(err);
    return Json(res).into_response();
  }

  match forward_write_to_leader(&app, &leader_addr, &command).await {
    Ok(res) => Json(res).into_response(),
    Err(message) => (StatusCode::BAD_GATEWAY, message).into_response(),
  }
}

async fn write_local(State(app): State<Arc<AppState>>, req: Json<SetCommand>) -> impl IntoResponse {
  let res = app.raft.client_write(req.0).await;
  Json(res)
}

async fn read(State(app): State<Arc<AppState>>, req: Json<String>) -> Response {
  let key = req.0;
  match app.state_machine_store.dump_state().await {
    Ok(state) => Json(state.get(&key).cloned()).into_response(),
    Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
  }
}

async fn linearizable_read(
  State(app): State<Arc<AppState>>,
  req: Json<String>,
) -> impl IntoResponse {
  let key = req.0;
  let res = async {
    let linearizer = app
      .raft
      .get_read_linearizer(ReadPolicy::ReadIndex)
      .await
      .map_err(|e| format!("{e:?}"))?;
    linearizer
      .await_ready(&app.raft)
      .await
      .map_err(|e| format!("{e:?}"))?;

    let state = app
      .state_machine_store
      .dump_state()
      .await
      .map_err(|e| e.to_string())?;
    Ok::<_, String>(state.get(&key).cloned())
  }
  .await;

  Json(res)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn actor_set_returns_old_value() {
    let actor_ref = KvStoreActor::spawn_default();

    let first = actor_ref
      .ask(SetCommand::new("color", "blue"))
      .send()
      .await
      .expect("actor should accept first set");
    assert_eq!(first, None);

    let second = actor_ref
      .ask(SetCommand::new("color", "green"))
      .send()
      .await
      .expect("actor should accept second set");
    assert_eq!(second, Some("blue".to_string()));
  }

  #[tokio::test]
  async fn state_machine_can_snapshot_actor_state() {
    let mut store = RaftStateMachineStore::spawn_actor();

    store
      .actor_ref()
      .ask(SetCommand::new("answer", "42"))
      .send()
      .await
      .expect("actor should accept set");

    let snapshot = store
      .build_snapshot()
      .await
      .expect("snapshot should be built");
    let decoded: BTreeMap<String, String> =
      serde_json::from_slice(snapshot.snapshot.get_ref()).expect("snapshot should decode");

    assert_eq!(decoded.get("answer"), Some(&"42".to_string()));
  }
}
