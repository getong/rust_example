use std::{
  collections::{HashMap, HashSet},
  error::Error,
  fmt,
  hash::Hash,
  sync::Arc,
  time::Duration,
};

use futures::StreamExt;
use libp2p::{
  Multiaddr, PeerId, Swarm, gossipsub,
  kad::{self, store::MemoryStore},
  mdns, ping,
  request_response::{self, OutboundRequestId, ResponseChannel},
  swarm::{
    ConnectionId, NetworkBehaviour, SwarmEvent,
    dial_opts::{DialOpts, PeerCondition},
  },
};
use prost::Message;
use tokio::sync::{mpsc, oneshot};

use crate::{
  Unreachable,
  network::{
    dispatcher::SwarmRequestDispatcher,
    openraft_sync::{OpenRaftSnapshotPartial, OpenRaftSyncState, group_id_string, sync_topic_hash},
    proto_codec::{ProstCodec, ProtoCodec, SerdeCodec},
    rpc::{RaftRpcRequest, RaftRpcResponse},
    transport::Libp2pNetworkFactory,
  },
  proto::raft_kv::{
    ChatMessage, ErrorResponse, NodeAnnouncement, RaftKvRequest, RaftKvResponse,
    raft_kv_response::Op as KvResponseOp,
  },
  signal::ShutdownRx,
  sqlite_sync_rpc::{SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage},
  tasks::rpc::{TaskRpcRequestMessage, TaskRpcResponseMessage},
};

pub const GOSSIP_TOPIC: &str = "openraft/cluster/1";
pub const NODE_ANNOUNCE_TOPIC: &str = "openraft/node-announce/1";
pub const OPENRAFT_CLUSTER_PROVIDER_KEY: &str = "openraft_cluster";
pub const RAFT_RPC_PROTOCOL: &str = "/openraft/raft/1";
pub const KV_RPC_PROTOCOL: &str = "/openraft/kv/1";
pub const SQLITE_SYNC_RPC_PROTOCOL: &str = "/openraft/sqlite-sync/1";
pub const TASK_RPC_PROTOCOL: &str = "/openraft/task/1";
pub const KAD_PROTOCOL: &str = "/openraft/kad/1";
const DIAL_RETRY_BACKOFF: Duration = Duration::from_secs(2);
/// Consecutive ping failures on one connection before it is force-closed.
/// A hung peer (SIGSTOP, long GC pause) keeps its TCP connection alive at
/// the kernel level, so peers still see it as "connected" even though it
/// answers nothing. The membership guard keys voter liveness off libp2p
/// connectedness — without this cutoff a hung voter is never replaced.
const PING_FAILURE_DISCONNECT_THRESHOLD: u32 = 3;
const RECONNECT_RETRY_BACKOFF: Duration = Duration::from_secs(30);
const OUTGOING_FAILURE_LOG_BACKOFF: Duration = Duration::from_secs(30);
const OPENRAFT_SNAPSHOT_SYNC_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone)]
pub struct NetErr(pub String);

impl fmt::Display for NetErr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl Error for NetErr {}

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourEvent")]
pub struct Behaviour {
  pub raft_rpc: request_response::Behaviour<ProtoCodec>,
  pub kv_rpc: request_response::Behaviour<ProstCodec<RaftKvRequest, RaftKvResponse>>,
  pub sqlite_sync_rpc: request_response::Behaviour<
    SerdeCodec<SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage>,
  >,
  pub task_rpc:
    request_response::Behaviour<SerdeCodec<TaskRpcRequestMessage, TaskRpcResponseMessage>>,
  pub gossipsub: gossipsub::Behaviour,
  pub ping: ping::Behaviour,
  pub mdns: mdns::tokio::Behaviour,
  pub kad: kad::Behaviour<MemoryStore>,
}

#[derive(Debug)]
pub enum BehaviourEvent {
  Raft(request_response::Event<RaftRpcRequest, RaftRpcResponse>),
  Kv(request_response::Event<RaftKvRequest, RaftKvResponse>),
  SqliteSync(request_response::Event<SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage>),
  TaskRpc(request_response::Event<TaskRpcRequestMessage, TaskRpcResponseMessage>),
  Gossipsub(gossipsub::Event),
  Ping(ping::Event),
  Mdns(mdns::Event),
  Kad(kad::Event),
}

impl From<request_response::Event<RaftRpcRequest, RaftRpcResponse>> for BehaviourEvent {
  fn from(value: request_response::Event<RaftRpcRequest, RaftRpcResponse>) -> Self {
    Self::Raft(value)
  }
}

impl From<request_response::Event<RaftKvRequest, RaftKvResponse>> for BehaviourEvent {
  fn from(value: request_response::Event<RaftKvRequest, RaftKvResponse>) -> Self {
    Self::Kv(value)
  }
}

impl From<request_response::Event<SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage>>
  for BehaviourEvent
{
  fn from(
    value: request_response::Event<SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage>,
  ) -> Self {
    Self::SqliteSync(value)
  }
}

impl From<request_response::Event<TaskRpcRequestMessage, TaskRpcResponseMessage>>
  for BehaviourEvent
{
  fn from(value: request_response::Event<TaskRpcRequestMessage, TaskRpcResponseMessage>) -> Self {
    Self::TaskRpc(value)
  }
}

impl From<gossipsub::Event> for BehaviourEvent {
  fn from(value: gossipsub::Event) -> Self {
    Self::Gossipsub(value)
  }
}

impl From<ping::Event> for BehaviourEvent {
  fn from(value: ping::Event) -> Self {
    Self::Ping(value)
  }
}

impl From<mdns::Event> for BehaviourEvent {
  fn from(value: mdns::Event) -> Self {
    Self::Mdns(value)
  }
}

impl From<kad::Event> for BehaviourEvent {
  fn from(value: kad::Event) -> Self {
    Self::Kad(value)
  }
}

pub enum Command {
  SetKadMode {
    mode: kad::Mode,
  },
  StartProviding {
    key: String,
    resp: oneshot::Sender<Result<(), NetErr>>,
  },
  LeaveKad {
    provider_keys: Vec<String>,
    resp: oneshot::Sender<Result<(), NetErr>>,
  },
  GetProviders {
    key: String,
    resp: oneshot::Sender<Result<HashSet<PeerId>, NetErr>>,
  },
  Dial {
    addr: Multiaddr,
  },
  EnsureConnection {
    peer: PeerId,
    addr: Multiaddr,
    resp: oneshot::Sender<Result<(), NetErr>>,
  },
  EnsureConnectionAny {
    peer: PeerId,
    resp: oneshot::Sender<Result<(), NetErr>>,
  },
  GossipsubPublish {
    topic: String,
    data: Vec<u8>,
  },
  GetLibp2pInfo {
    resp: oneshot::Sender<Libp2pSwarmReport>,
  },
  PublishOpenRaftSnapshot {
    group_id: String,
    resp: oneshot::Sender<Result<String, NetErr>>,
  },
  /// Internal follow-up to `PublishOpenRaftSnapshot`: the snapshot partial
  /// was built off-loop (it can take seconds for a large state machine) and
  /// is ready to be published without stalling swarm event processing.
  PublishOpenRaftSnapshotBuilt {
    partial: OpenRaftSnapshotPartial,
    resp: oneshot::Sender<Result<String, NetErr>>,
  },
  RaftRequest {
    peer: PeerId,
    req: RaftRpcRequest,
    resp: oneshot::Sender<Result<RaftRpcResponse, NetErr>>,
  },
  RaftRespond {
    channel: ResponseChannel<RaftRpcResponse>,
    resp: RaftRpcResponse,
  },
  KvRequest {
    peer: PeerId,
    req: RaftKvRequest,
    resp: oneshot::Sender<Result<RaftKvResponse, NetErr>>,
  },
  KvRespond {
    channel: ResponseChannel<RaftKvResponse>,
    resp: RaftKvResponse,
  },
  SqliteSyncRequest {
    peer: PeerId,
    req: SqliteSyncRpcRequestMessage,
    resp: oneshot::Sender<Result<SqliteSyncRpcResponseMessage, NetErr>>,
  },
  SqliteSyncRespond {
    channel: ResponseChannel<SqliteSyncRpcResponseMessage>,
    resp: SqliteSyncRpcResponseMessage,
  },
  TaskRpcRequest {
    peer: PeerId,
    req: TaskRpcRequestMessage,
    resp: oneshot::Sender<Result<TaskRpcResponseMessage, NetErr>>,
  },
  TaskRpcRespond {
    channel: ResponseChannel<TaskRpcResponseMessage>,
    resp: TaskRpcResponseMessage,
  },
}

/// Live gossipsub state for one subscribed topic.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GossipTopicReport {
  pub topic: String,
  /// Peers in this node's gossipsub mesh for the topic.
  pub mesh_peers: usize,
  /// Known peers subscribed to the topic (mesh + fanout candidates).
  pub subscribed_peers: usize,
}

/// Snapshot of the live libp2p swarm state, collected inside the swarm loop.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Libp2pSwarmReport {
  pub local_peer_id: String,
  /// Actual listening multiaddrs (after port/interface resolution).
  pub listeners: Vec<String>,
  pub connected_peers: usize,
  pub established_connections: u32,
  /// Kademlia mode: "Server" (control nodes advertise) or "Client" (workers).
  pub kad_mode: String,
  /// Peers currently in the kademlia routing table.
  pub kad_routing_table_peers: usize,
  pub gossipsub_topics: Vec<GossipTopicReport>,
  /// All peers the gossipsub behaviour knows about.
  pub gossipsub_known_peers: usize,
}

#[derive(Clone)]
pub struct Libp2pClient {
  tx: mpsc::Sender<Command>,
  timeout: Duration,
}

impl Libp2pClient {
  pub fn new(tx: mpsc::Sender<Command>, timeout: Duration) -> Self {
    Self { tx, timeout }
  }

  pub async fn set_kad_mode(&self, mode: kad::Mode) {
    let _ = self.tx.send(Command::SetKadMode { mode }).await;
  }

  /// Fetch a snapshot of the live swarm state (listeners, kad mode,
  /// gossipsub topics, connection counts).
  pub async fn libp2p_info(&self) -> Result<Libp2pSwarmReport, NetErr> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::GetLibp2pInfo { resp: resp_tx })
      .await
      .map_err(|e| NetErr(format!("command channel closed: {e}")))?;

    tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| NetErr(format!("timeout: {e}")))
      .and_then(|r| r.map_err(|e| NetErr(format!("libp2p info dropped: {e}"))))
  }

  pub async fn start_providing(&self, key: String) -> Result<(), NetErr> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::StartProviding { key, resp: resp_tx })
      .await
      .map_err(|e| NetErr(format!("command channel closed: {e}")))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| NetErr(format!("timeout: {e}")))
      .and_then(|r| r.map_err(|e| NetErr(format!("connect dropped: {e}"))))?;
    resp
  }

  pub async fn leave_openraft_kad(&self) -> Result<(), NetErr> {
    self
      .leave_kad(vec![OPENRAFT_CLUSTER_PROVIDER_KEY.to_string()])
      .await
  }

  pub async fn leave_kad(&self, provider_keys: Vec<String>) -> Result<(), NetErr> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::LeaveKad {
        provider_keys,
        resp: resp_tx,
      })
      .await
      .map_err(|e| NetErr(format!("command channel closed: {e}")))?;

    tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| NetErr(format!("timeout: {e}")))
      .and_then(|r| r.map_err(|e| NetErr(format!("leave_kad dropped: {e}"))))?
  }

  pub async fn get_providers(&self, key: String) -> Result<HashSet<PeerId>, NetErr> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::GetProviders { key, resp: resp_tx })
      .await
      .map_err(|e| NetErr(format!("command channel closed: {e}")))?;

    let resp = tokio::time::timeout(self.timeout * 5, resp_rx)
      .await
      .map_err(|e| NetErr(format!("timeout: {e}")))
      .and_then(|r| r.map_err(|e| NetErr(format!("connect dropped: {e}"))))?;
    resp
  }

  pub async fn dial(&self, addr: Multiaddr) {
    let _ = self.tx.send(Command::Dial { addr }).await;
  }

  pub async fn connect(&self, peer: PeerId, addr: Multiaddr) -> Result<(), Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::EnsureConnection {
        peer,
        addr,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("connect timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("connect dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }

  pub async fn connect_any(&self, peer: PeerId) -> Result<(), Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::EnsureConnectionAny {
        peer,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("connect timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("connect dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }

  pub async fn publish_gossipsub(&self, topic: &str, data: Vec<u8>) -> Result<(), NetErr> {
    self
      .tx
      .send(Command::GossipsubPublish {
        topic: topic.to_string(),
        data,
      })
      .await
      .map_err(|e| NetErr(format!("command channel closed: {e}")))
  }

  pub async fn publish_openraft_snapshot(&self, group_id: String) -> Result<String, NetErr> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::PublishOpenRaftSnapshot {
        group_id,
        resp: resp_tx,
      })
      .await
      .map_err(|e| NetErr(format!("command channel closed: {e}")))?;

    let timeout = self.timeout.max(OPENRAFT_SNAPSHOT_SYNC_TIMEOUT);
    tokio::time::timeout(timeout, resp_rx)
      .await
      .map_err(|e| NetErr(format!("openraft snapshot sync timeout: {e}")))?
      .map_err(|e| NetErr(format!("openraft snapshot sync dropped: {e}")))?
  }

  pub async fn request(
    &self,
    peer: PeerId,
    req: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::RaftRequest {
        peer,
        req,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("request timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("response dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }
}

#[derive(Clone)]
pub struct KvClient {
  tx: mpsc::Sender<Command>,
  timeout: Duration,
}

impl KvClient {
  pub fn new(tx: mpsc::Sender<Command>, timeout: Duration) -> Self {
    Self { tx, timeout }
  }

  pub async fn publish_gossipsub(&self, topic: &str, data: Vec<u8>) {
    let _ = self
      .tx
      .send(Command::GossipsubPublish {
        topic: topic.to_string(),
        data,
      })
      .await;
  }

  pub async fn dial(&self, addr: Multiaddr) {
    let _ = self.tx.send(Command::Dial { addr }).await;
  }

  pub async fn connect(&self, peer: PeerId, addr: Multiaddr) -> Result<(), Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::EnsureConnection {
        peer,
        addr,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("connect timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("connect dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }

  pub async fn connect_any(&self, peer: PeerId) -> Result<(), Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::EnsureConnectionAny {
        peer,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("connect timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("connect dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }

  pub async fn request(
    &self,
    peer: PeerId,
    req: RaftKvRequest,
  ) -> Result<RaftKvResponse, Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::KvRequest {
        peer,
        req,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("request timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("response dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }
}

#[derive(Clone)]
pub struct SqliteSyncClient {
  tx: mpsc::Sender<Command>,
  timeout: Duration,
}

impl SqliteSyncClient {
  pub fn new(tx: mpsc::Sender<Command>, timeout: Duration) -> Self {
    Self { tx, timeout }
  }

  pub async fn connect(&self, peer: PeerId, addr: Multiaddr) -> Result<(), Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::EnsureConnection {
        peer,
        addr,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("connect timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("connect dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }

  pub async fn connect_any(&self, peer: PeerId) -> Result<(), Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::EnsureConnectionAny {
        peer,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("connect timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("connect dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }

  pub async fn request(
    &self,
    peer: PeerId,
    req: SqliteSyncRpcRequestMessage,
  ) -> Result<SqliteSyncRpcResponseMessage, Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::SqliteSyncRequest {
        peer,
        req,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("request timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("response dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }
}

/// Client handle for the tarpc-based task RPC protocol (`/openraft/task/1`).
#[derive(Clone)]
pub struct TaskRpcClient {
  tx: mpsc::Sender<Command>,
  timeout: Duration,
}

impl TaskRpcClient {
  pub fn new(tx: mpsc::Sender<Command>, timeout: Duration) -> Self {
    Self { tx, timeout }
  }

  pub async fn connect(&self, peer: PeerId, addr: Multiaddr) -> Result<(), Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::EnsureConnection {
        peer,
        addr,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("connect timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("connect dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }

  pub async fn connect_any(&self, peer: PeerId) -> Result<(), Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::EnsureConnectionAny {
        peer,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("connect timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("connect dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }

  pub async fn request(
    &self,
    peer: PeerId,
    req: TaskRpcRequestMessage,
  ) -> Result<TaskRpcResponseMessage, Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::TaskRpcRequest {
        peer,
        req,
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("request timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("response dropped: {e}")))))?;

    resp.map_err(|e| Unreachable::new(&e))
  }
}

/// Full-node swarm loop.
///
/// The loop owns the `Swarm` exclusively: every other component interacts
/// with it through the `Command` mpsc channel, never through a shared lock.
/// Long-running work (request dispatch, snapshot building/installing) is
/// spawned onto separate tasks so the loop stays a thin, non-blocking
/// multiplexer over swarm events and commands.
pub async fn run_swarm(
  mut swarm: Swarm<Behaviour>,
  mut cmd_rx: mpsc::Receiver<Command>,
  cmd_tx: mpsc::Sender<Command>,
  network: Libp2pNetworkFactory,
  dispatcher: Arc<dyn SwarmRequestDispatcher>,
  mut shutdown_rx: ShutdownRx,
) {
  let mut pending_raft: HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<RaftRpcResponse, NetErr>>,
  > = HashMap::new();
  let mut pending_kv: HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>> =
    HashMap::new();
  let mut pending_sqlite_sync: HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<SqliteSyncRpcResponseMessage, NetErr>>,
  > = HashMap::new();
  let mut pending_task_rpc: HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<TaskRpcResponseMessage, NetErr>>,
  > = HashMap::new();
  let mut pending_connect: HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>> =
    HashMap::new();
  let mut openraft_sync = OpenRaftSyncState::default();
  let mut dial_backoff_until: HashMap<PeerId, tokio::time::Instant> = HashMap::new();
  let mut reconnect_backoff_until: HashMap<PeerId, tokio::time::Instant> = HashMap::new();
  let mut outgoing_failure_log_backoff_until: HashMap<PeerId, tokio::time::Instant> =
    HashMap::new();
  let mut connected_peers: HashSet<PeerId> = HashSet::new();
  let mut pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<Result<(), NetErr>>> =
    HashMap::new();
  let mut pending_get_providers: HashMap<kad::QueryId, GetProvidersState> = HashMap::new();
  let mut ping_failures: HashMap<ConnectionId, u32> = HashMap::new();
  let mut reconnect_tick = tokio::time::interval(Duration::from_secs(12));
  reconnect_tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm");
        leave_openraft_kad(&mut swarm);
        fail_pending_swarm_requests(
          &mut pending_raft,
          &mut pending_kv,
          &mut pending_sqlite_sync,
          &mut pending_task_rpc,
          &mut pending_connect,
          &mut pending_start_providing,
          &mut pending_get_providers,
          "swarm shutting down",
        );
        break;
      }
      _ = reconnect_tick.tick() => {
        handle_reconnect_tick(
          &mut swarm,
          &network,
          &connected_peers,
          &mut reconnect_backoff_until,
        ).await;
      }
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else {
          tracing::warn!("swarm command channel closed; stopping swarm");
          leave_openraft_kad(&mut swarm);
          fail_pending_swarm_requests(
            &mut pending_raft,
            &mut pending_kv,
            &mut pending_sqlite_sync,
            &mut pending_task_rpc,
            &mut pending_connect,
            &mut pending_start_providing,
            &mut pending_get_providers,
            "swarm command channel closed",
          );
          return;
        };
        if let Command::PublishOpenRaftSnapshot { group_id, resp } = cmd {
          // Building the partial reads the raft snapshot and can take
          // seconds; do it off-loop and publish via the follow-up command.
          spawn_build_openraft_snapshot(cmd_tx.clone(), group_id, resp);
          continue;
        }
        if let Command::PublishOpenRaftSnapshotBuilt { partial, resp } = cmd {
          publish_openraft_snapshot_partial(&mut swarm, partial, resp, &mut openraft_sync);
          continue;
        }

        handle_command(
          &mut swarm,
          cmd,
          &mut pending_raft,
          &mut pending_kv,
          &mut pending_sqlite_sync,
          &mut pending_task_rpc,
          &mut pending_connect,
          &mut dial_backoff_until,
          &mut pending_start_providing,
          &mut pending_get_providers,
        );
      }

      ev = swarm.next() => {
        let Some(ev) = ev else {
          tracing::warn!("swarm stream ended; stopping swarm");
          fail_pending_swarm_requests(
            &mut pending_raft,
            &mut pending_kv,
            &mut pending_sqlite_sync,
            &mut pending_task_rpc,
            &mut pending_connect,
            &mut pending_start_providing,
            &mut pending_get_providers,
            "swarm stream ended",
          );
          break;
        };
        handle_swarm_event(
          &mut swarm,
          ev,
          &network,
          dispatcher.clone(),
          &cmd_tx,
          &mut pending_raft,
          &mut pending_kv,
          &mut pending_sqlite_sync,
          &mut pending_task_rpc,
          &mut pending_connect,
          &mut openraft_sync,
          &mut connected_peers,
          &mut dial_backoff_until,
          &mut reconnect_backoff_until,
          &mut outgoing_failure_log_backoff_until,
          &mut pending_start_providing,
          &mut pending_get_providers,
          &mut ping_failures,
        )
        .await;
      }
    }
  }
}

/// Proactively redial disconnected peers that must stay connected: configured
/// members, bootstrap nodes, and active raft RPC targets (see
/// `Libp2pNetworkFactory::reconnect_targets`). Deliberately NOT all of
/// `known_nodes`: redialing every announced/discovered node would build an
/// O(N^2) full mesh across the cluster and exhaust file descriptors at scale.
/// Non-pinned peers are dialed on demand by the RPC paths and reaped by the
/// swarm idle-connection timeout.
async fn handle_reconnect_tick(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  connected_peers: &HashSet<PeerId>,
  reconnect_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
) {
  let nodes = network.reconnect_targets().await;
  let now = tokio::time::Instant::now();
  for (_node_id, peer_id, addr) in nodes {
    if peer_id == *swarm.local_peer_id() {
      continue;
    }
    if connected_peers.contains(&peer_id) {
      continue;
    }
    if let Some(until) = reconnect_backoff_until.get(&peer_id).copied() {
      if now < until {
        tracing::debug!(
          peer = %peer_id,
          addr = %addr,
          retry_in_ms = (until - now).as_millis(),
          "automatic reconnect backoff active"
        );
        continue;
      }
      reconnect_backoff_until.remove(&peer_id);
    }
    tracing::debug!(
      peer = %peer_id,
      addr = %addr,
      "reconnecting to peer"
    );
    dial_peer_addr(swarm, addr.clone());
    add_kad_address_from_p2p(swarm, &addr);
    reconnect_backoff_until.insert(peer_id, now + RECONNECT_RETRY_BACKOFF);
  }
}

struct GetProvidersState {
  providers: HashSet<PeerId>,
  resp: oneshot::Sender<Result<HashSet<PeerId>, NetErr>>,
}

fn collect_swarm_report(swarm: &mut Swarm<Behaviour>) -> Libp2pSwarmReport {
  let local_peer_id = swarm.local_peer_id().to_string();
  let listeners: Vec<String> = swarm.listeners().map(|addr| addr.to_string()).collect();
  let network_info = swarm.network_info();
  let connected_peers = network_info.num_peers();
  let established_connections = network_info.connection_counters().num_established();

  let behaviour = swarm.behaviour_mut();
  let kad_mode = format!("{:?}", behaviour.kad.mode());
  let kad_routing_table_peers: usize = behaviour
    .kad
    .kbuckets()
    .map(|bucket| bucket.num_entries())
    .sum();

  // Per-topic mesh and subscription counts.
  let topics: Vec<gossipsub::TopicHash> = behaviour.gossipsub.topics().cloned().collect();
  let peer_topics: Vec<Vec<gossipsub::TopicHash>> = behaviour
    .gossipsub
    .all_peers()
    .map(|(_, topics)| topics.into_iter().cloned().collect())
    .collect();
  let gossipsub_known_peers = peer_topics.len();
  let gossipsub_topics = topics
    .iter()
    .map(|hash| GossipTopicReport {
      topic: hash.to_string(),
      mesh_peers: behaviour.gossipsub.mesh_peers(hash).count(),
      subscribed_peers: peer_topics
        .iter()
        .filter(|topics| topics.contains(hash))
        .count(),
    })
    .collect();

  Libp2pSwarmReport {
    local_peer_id,
    listeners,
    connected_peers,
    established_connections,
    kad_mode,
    kad_routing_table_peers,
    gossipsub_topics,
    gossipsub_known_peers,
  }
}

fn handle_command(
  swarm: &mut Swarm<Behaviour>,
  cmd: Command,
  pending_raft: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftRpcResponse, NetErr>>>,
  pending_kv: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>>,
  pending_sqlite_sync: &mut HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<SqliteSyncRpcResponseMessage, NetErr>>,
  >,
  pending_task_rpc: &mut HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<TaskRpcResponseMessage, NetErr>>,
  >,
  pending_connect: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  pending_start_providing: &mut HashMap<kad::QueryId, oneshot::Sender<Result<(), NetErr>>>,
  pending_get_providers: &mut HashMap<kad::QueryId, GetProvidersState>,
) {
  match cmd {
    Command::SetKadMode { mode } => {
      swarm.behaviour_mut().kad.set_mode(Some(mode));
      tracing::info!(mode = ?mode, "set kademlia mode");
    }
    Command::StartProviding { key, resp } => {
      let record_key = kad::RecordKey::new(&key);
      match swarm.behaviour_mut().kad.start_providing(record_key) {
        Ok(query_id) => {
          pending_start_providing.insert(query_id, resp);
        }
        Err(e) => {
          let _ = resp.send(Err(NetErr(format!("start_providing failed: {:?}", e))));
        }
      }
    }
    Command::LeaveKad {
      provider_keys,
      resp,
    } => {
      leave_kad(swarm, &provider_keys);
      let _ = resp.send(Ok(()));
    }
    Command::GetProviders { key, resp } => {
      let record_key = kad::RecordKey::new(&key);
      let query_id = swarm.behaviour_mut().kad.get_providers(record_key);
      pending_get_providers.insert(
        query_id,
        GetProvidersState {
          providers: HashSet::new(),
          resp,
        },
      );
    }
    Command::Dial { addr } => {
      dial_peer_addr(swarm, addr.clone());
      // Inserting the address triggers kad's automatic (throttled)
      // bootstrap; no manual query kick is needed.
      add_kad_address_from_p2p(swarm, &addr);
    }
    Command::EnsureConnection { peer, addr, resp } => {
      ensure_peer_connection(
        swarm,
        pending_connect,
        dial_backoff_until,
        peer,
        Some(addr),
        resp,
      );
    }
    Command::EnsureConnectionAny { peer, resp } => {
      ensure_peer_connection(swarm, pending_connect, dial_backoff_until, peer, None, resp);
    }
    Command::GossipsubPublish { topic, data } => {
      let topic = gossipsub::IdentTopic::new(topic);
      if let Err(err) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
        tracing::warn!("gossipsub publish failed: {err}");
      }
    }
    Command::GetLibp2pInfo { resp } => {
      let _ = resp.send(collect_swarm_report(swarm));
    }
    Command::PublishOpenRaftSnapshot { resp, .. }
    | Command::PublishOpenRaftSnapshotBuilt { resp, .. } => {
      let _ = resp.send(Err(NetErr(
        "openraft snapshot sync is not available in this swarm loop".to_string(),
      )));
    }
    Command::RaftRequest { peer, req, resp } => {
      let id = swarm.behaviour_mut().raft_rpc.send_request(&peer, req);
      pending_raft.insert(id, resp);
    }
    Command::RaftRespond { channel, resp } => {
      let _ = swarm.behaviour_mut().raft_rpc.send_response(channel, resp);
    }
    Command::KvRequest { peer, req, resp } => {
      let id = swarm.behaviour_mut().kv_rpc.send_request(&peer, req);
      pending_kv.insert(id, resp);
    }
    Command::KvRespond { channel, resp } => {
      let _ = swarm.behaviour_mut().kv_rpc.send_response(channel, resp);
    }
    Command::SqliteSyncRequest { peer, req, resp } => {
      let id = swarm
        .behaviour_mut()
        .sqlite_sync_rpc
        .send_request(&peer, req);
      pending_sqlite_sync.insert(id, resp);
    }
    Command::SqliteSyncRespond { channel, resp } => {
      let _ = swarm
        .behaviour_mut()
        .sqlite_sync_rpc
        .send_response(channel, resp);
    }
    Command::TaskRpcRequest { peer, req, resp } => {
      let id = swarm.behaviour_mut().task_rpc.send_request(&peer, req);
      pending_task_rpc.insert(id, resp);
    }
    Command::TaskRpcRespond { channel, resp } => {
      let _ = swarm.behaviour_mut().task_rpc.send_response(channel, resp);
    }
  }
}

/// Build the snapshot partial on a separate task and hand it back to the
/// swarm loop as `Command::PublishOpenRaftSnapshotBuilt`. Building reads the
/// raft snapshot (disk + serialization) and must not stall the swarm loop.
fn spawn_build_openraft_snapshot(
  cmd_tx: mpsc::Sender<Command>,
  group_id: String,
  resp: oneshot::Sender<Result<String, NetErr>>,
) {
  tokio::spawn(async move {
    let partial = match OpenRaftSnapshotPartial::from_raft_group(&group_id).await {
      Ok(Some(partial)) => partial,
      Ok(None) => {
        let _ = resp.send(Err(NetErr(format!(
          "openraft group {group_id} has no snapshot"
        ))));
        return;
      }
      Err(err) => {
        let _ = resp.send(Err(NetErr(format!(
          "build openraft snapshot partial failed: {err}"
        ))));
        return;
      }
    };

    if cmd_tx
      .send(Command::PublishOpenRaftSnapshotBuilt { partial, resp })
      .await
      .is_err()
    {
      tracing::warn!(
        group_id = %group_id,
        "swarm loop stopped before the built openraft snapshot could be published"
      );
    }
  });
}

fn publish_openraft_snapshot_partial(
  swarm: &mut Swarm<Behaviour>,
  partial: OpenRaftSnapshotPartial,
  resp: oneshot::Sender<Result<String, NetErr>>,
  openraft_sync: &mut OpenRaftSyncState,
) {
  let sync_group = group_id_string(&partial.group_id);
  let topic = sync_topic_hash();
  let publish_result = swarm
    .behaviour_mut()
    .gossipsub
    .publish_partial(topic, partial.clone());

  match publish_result {
    Ok(()) => {
      openraft_sync.insert_local(partial);
      let _ = resp.send(Ok(sync_group));
    }
    Err(err) => {
      let _ = resp.send(Err(NetErr(format!(
        "publish openraft snapshot partial failed: {err}"
      ))));
    }
  }
}

async fn handle_swarm_event(
  swarm: &mut Swarm<Behaviour>,
  event: SwarmEvent<BehaviourEvent>,
  network: &Libp2pNetworkFactory,
  dispatcher: Arc<dyn SwarmRequestDispatcher>,
  cmd_tx: &mpsc::Sender<Command>,
  pending_raft: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftRpcResponse, NetErr>>>,
  pending_kv: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>>,
  pending_sqlite_sync: &mut HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<SqliteSyncRpcResponseMessage, NetErr>>,
  >,
  pending_task_rpc: &mut HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<TaskRpcResponseMessage, NetErr>>,
  >,
  pending_connect: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  openraft_sync: &mut OpenRaftSyncState,
  connected_peers: &mut HashSet<PeerId>,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  reconnect_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  outgoing_failure_log_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  pending_start_providing: &mut HashMap<kad::QueryId, oneshot::Sender<Result<(), NetErr>>>,
  pending_get_providers: &mut HashMap<kad::QueryId, GetProvidersState>,
  ping_failures: &mut HashMap<ConnectionId, u32>,
) {
  match event {
    SwarmEvent::Behaviour(BehaviourEvent::Raft(event)) => {
      handle_raft_event(dispatcher.clone(), cmd_tx, pending_raft, event).await;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Kv(event)) => {
      handle_kv_event(dispatcher.clone(), cmd_tx, pending_kv, event);
    }
    SwarmEvent::Behaviour(BehaviourEvent::SqliteSync(event)) => {
      handle_sqlite_sync_event(dispatcher.clone(), cmd_tx, pending_sqlite_sync, event);
    }
    SwarmEvent::Behaviour(BehaviourEvent::TaskRpc(event)) => {
      handle_task_rpc_event(dispatcher.clone(), cmd_tx, pending_task_rpc, event);
    }
    SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => {
      handle_mdns_event(swarm, network, event).await;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(event)) => {
      handle_gossipsub_event(swarm, network, openraft_sync, event).await;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Ping(event)) => {
      handle_ping_event(swarm, ping_failures, event);
    }
    SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
      handle_kad_event(
        swarm,
        Some(network),
        Some(connected_peers),
        event,
        pending_start_providing,
        pending_get_providers,
      );
    }
    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
      handle_connection_established(
        swarm,
        pending_connect,
        connected_peers,
        dial_backoff_until,
        peer_id,
      );
      reconnect_backoff_until.remove(&peer_id);
      outgoing_failure_log_backoff_until.remove(&peer_id);
      network.set_peer_connected(peer_id).await;
    }
    SwarmEvent::ConnectionClosed {
      peer_id,
      connection_id,
      num_established,
      cause,
      ..
    } => {
      ping_failures.remove(&connection_id);
      handle_connection_closed(swarm, connected_peers, peer_id, num_established, cause);
      if num_established == 0 {
        network.set_peer_disconnected(peer_id).await;
      }
    }
    SwarmEvent::NewListenAddr { address, .. } => {
      tracing::info!("listening on {address}");
    }
    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
      handle_outgoing_connection_error(
        swarm,
        pending_connect,
        dial_backoff_until,
        outgoing_failure_log_backoff_until,
        peer_id,
        error,
      );
    }
    _ => {}
  }
}

async fn handle_raft_event(
  dispatcher: Arc<dyn SwarmRequestDispatcher>,
  cmd_tx: &mpsc::Sender<Command>,
  pending_raft: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftRpcResponse, NetErr>>>,
  event: request_response::Event<RaftRpcRequest, RaftRpcResponse>,
) {
  match event {
    request_response::Event::Message { message, .. } => match message {
      request_response::Message::Request {
        request, channel, ..
      } => {
        let dispatcher = dispatcher.clone();
        let tx = cmd_tx.clone();
        tokio::spawn(async move {
          let resp = dispatcher.handle_raft(request).await;
          let _ = tx.send(Command::RaftRespond { channel, resp }).await;
        });
      }
      request_response::Message::Response {
        request_id,
        response,
      } => {
        if let Some(tx) = pending_raft.remove(&request_id) {
          let _ = tx.send(Ok(response));
        }
      }
    },
    request_response::Event::OutboundFailure {
      request_id, error, ..
    } => {
      if let Some(tx) = pending_raft.remove(&request_id) {
        let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
      }
    }
    request_response::Event::InboundFailure { .. } => {}
    request_response::Event::ResponseSent { .. } => {}
  }
}

fn handle_kv_event(
  dispatcher: Arc<dyn SwarmRequestDispatcher>,
  cmd_tx: &mpsc::Sender<Command>,
  pending_kv: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>>,
  event: request_response::Event<RaftKvRequest, RaftKvResponse>,
) {
  match event {
    request_response::Event::Message { message, .. } => match message {
      request_response::Message::Request {
        request, channel, ..
      } => {
        let dispatcher = dispatcher.clone();
        let tx = cmd_tx.clone();
        tokio::spawn(async move {
          let resp = dispatcher.handle_kv(request).await;
          let _ = tx.send(Command::KvRespond { channel, resp }).await;
        });
      }
      request_response::Message::Response {
        request_id,
        response,
      } => {
        if let Some(tx) = pending_kv.remove(&request_id) {
          let _ = tx.send(Ok(response));
        }
      }
    },
    request_response::Event::OutboundFailure {
      request_id, error, ..
    } => {
      if let Some(tx) = pending_kv.remove(&request_id) {
        let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
      }
    }
    request_response::Event::InboundFailure { .. } => {}
    request_response::Event::ResponseSent { .. } => {}
  }
}

fn handle_sqlite_sync_event(
  dispatcher: Arc<dyn SwarmRequestDispatcher>,
  cmd_tx: &mpsc::Sender<Command>,
  pending_sqlite_sync: &mut HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<SqliteSyncRpcResponseMessage, NetErr>>,
  >,
  event: request_response::Event<SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage>,
) {
  match event {
    request_response::Event::Message { message, .. } => match message {
      request_response::Message::Request {
        request, channel, ..
      } => {
        let dispatcher = dispatcher.clone();
        let tx = cmd_tx.clone();
        tokio::spawn(async move {
          let resp = dispatcher.handle_sqlite_sync(request).await;
          let _ = tx.send(Command::SqliteSyncRespond { channel, resp }).await;
        });
      }
      request_response::Message::Response {
        request_id,
        response,
      } => {
        if let Some(tx) = pending_sqlite_sync.remove(&request_id) {
          let _ = tx.send(Ok(response));
        }
      }
    },
    request_response::Event::OutboundFailure {
      request_id, error, ..
    } => {
      if let Some(tx) = pending_sqlite_sync.remove(&request_id) {
        let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
      }
    }
    request_response::Event::InboundFailure { .. } => {}
    request_response::Event::ResponseSent { .. } => {}
  }
}

fn handle_task_rpc_event(
  dispatcher: Arc<dyn SwarmRequestDispatcher>,
  cmd_tx: &mpsc::Sender<Command>,
  pending_task_rpc: &mut HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<TaskRpcResponseMessage, NetErr>>,
  >,
  event: request_response::Event<TaskRpcRequestMessage, TaskRpcResponseMessage>,
) {
  match event {
    request_response::Event::Message { message, .. } => match message {
      request_response::Message::Request {
        request, channel, ..
      } => {
        let dispatcher = dispatcher.clone();
        let tx = cmd_tx.clone();
        tokio::spawn(async move {
          let resp = dispatcher.handle_task_rpc(request).await;
          let _ = tx.send(Command::TaskRpcRespond { channel, resp }).await;
        });
      }
      request_response::Message::Response {
        request_id,
        response,
      } => {
        if let Some(tx) = pending_task_rpc.remove(&request_id) {
          let _ = tx.send(Ok(response));
        }
      }
    },
    request_response::Event::OutboundFailure {
      request_id, error, ..
    } => {
      if let Some(tx) = pending_task_rpc.remove(&request_id) {
        let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
      }
    }
    request_response::Event::InboundFailure { .. } => {}
    request_response::Event::ResponseSent { .. } => {}
  }
}

async fn handle_mdns_event(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  event: mdns::Event,
) {
  match event {
    mdns::Event::Discovered(list) => {
      for (peer, addr) in list {
        if crate::network::transport::is_undialable_discovered_addr(&addr) {
          continue;
        }
        let mut use_discovered_addr = network.update_peer_addr_from_mdns(peer, addr.clone()).await;
        if network.register_discovered_peer(peer, addr.clone()).await {
          use_discovered_addr = true;
        }
        if use_discovered_addr {
          // New routing-table entries trigger kad's automatic bootstrap.
          add_kad_peer_address(swarm, peer, addr);
        }
        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
      }
    }
    mdns::Event::Expired(list) => {
      for (peer, addr) in list {
        let addr = strip_p2p(addr);
        swarm.behaviour_mut().kad.remove_address(&peer, &addr);
        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer);
      }
    }
  }
}

fn node_announce_topic_hash() -> gossipsub::TopicHash {
  gossipsub::IdentTopic::new(NODE_ANNOUNCE_TOPIC).hash()
}

fn task_assign_topic_hash() -> gossipsub::TopicHash {
  gossipsub::IdentTopic::new(crate::tasks::scheduler::TASK_ASSIGN_TOPIC).hash()
}

/// Handle a node self-announcement: (re)register the sender in the local
/// known-nodes address book and refresh its liveness timestamp. This is what
/// brings a node back into `known_nodes` after it crashed, was pruned, and
/// restarted — mdns re-discovery alone can lag by minutes.
///
/// Registration only: announcements never trigger a dial. Every node hears
/// every announcement, so dialing here would have each node open connections
/// to the whole cluster (O(N^2) connections in total).
async fn handle_node_announcement(network: &Libp2pNetworkFactory, data: &[u8]) {
  let announcement = match NodeAnnouncement::decode(data) {
    Ok(announcement) => announcement,
    Err(err) => {
      tracing::debug!(error = %err, "invalid node announcement message");
      return;
    }
  };

  if let Err(err) = network
    .register_announced_node(
      crate::NodeId::new(&announcement.node_id),
      &announcement.addr,
    )
    .await
  {
    tracing::debug!(
      node_id = %announcement.node_id,
      addr = %announcement.addr,
      error = ?err,
      "failed to register announced node"
    );
  }
}

async fn handle_gossipsub_event(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  openraft_sync: &mut OpenRaftSyncState,
  event: gossipsub::Event,
) {
  match event {
    gossipsub::Event::Message {
      propagation_source,
      message_id,
      message,
    } => {
      if message.topic == node_announce_topic_hash() {
        handle_node_announcement(network, message.data.as_slice()).await;
        return;
      }

      if message.topic == task_assign_topic_hash() {
        match crate::proto::raft_kv::TaskAssignedMessage::decode(message.data.as_slice()) {
          Ok(assigned) => {
            tracing::debug!(
              task_id = %assigned.task_id,
              worker_id = %assigned.worker_id,
              "task assignment gossip received"
            );
            crate::tasks::worker::notify_assignment(
              &assigned.worker_id,
              &assigned.task_id,
              assigned.lease_epoch,
            );
          }
          Err(err) => tracing::debug!(error = %err, "invalid task assignment message"),
        }
        return;
      }

      match ChatMessage::decode(message.data.as_slice()) {
        Ok(chat) => {
          tracing::info!(
            peer = %propagation_source,
            message_id = %message_id,
            from = %chat.from,
            text = %chat.text,
            ts = chat.ts_unix_ms,
            "chat message"
          );
        }
        Err(err) => {
          tracing::info!(
            peer = %propagation_source,
            message_id = %message_id,
            len = message.data.len(),
            error = %err,
            "gossipsub message (decode failed)"
          );
        }
      }
    }
    gossipsub::Event::Partial {
      topic_hash,
      peer_id,
      group_id,
      message,
      metadata,
    } => {
      if topic_hash != sync_topic_hash() {
        tracing::debug!(peer = %peer_id, topic = %topic_hash, "partial message on unknown topic");
        return;
      }

      let Some(metadata) = metadata else {
        tracing::warn!(peer = %peer_id, group = %group_id_string(&group_id), "openraft snapshot partial missing metadata");
        swarm
          .behaviour_mut()
          .gossipsub
          .report_invalid_partial(peer_id, &topic_hash);
        return;
      };

      let update =
        match openraft_sync.handle_partial(group_id.clone(), &metadata, message.as_deref()) {
          Ok(update) => update,
          Err(err) => {
            tracing::warn!(
              peer = %peer_id,
              group = %group_id_string(&group_id),
              error = ?err,
              "invalid openraft snapshot partial"
            );
            swarm
              .behaviour_mut()
              .gossipsub
              .report_invalid_partial(peer_id, &topic_hash);
            return;
          }
        };

      if update.should_republish {
        if let Err(err) = swarm
          .behaviour_mut()
          .gossipsub
          .publish_partial(topic_hash.clone(), update.partial.clone())
        {
          tracing::debug!(error = ?err, "republish openraft snapshot partial failed");
        }
      }

      if update.first_complete {
        // Installing a snapshot feeds it through the raft state machine and
        // can take seconds; run it off-loop so swarm event processing (raft
        // RPCs included) is not stalled behind it.
        let partial = update.partial;
        tokio::spawn(async move {
          let raft_group_id = partial.raft_group_id.clone();
          let snapshot_id = partial.snapshot_id.clone();
          match partial.install().await {
            Ok(resp) => {
              tracing::info!(
                peer = %peer_id,
                group = %raft_group_id,
                snapshot_id = %snapshot_id,
                response = ?resp,
                "installed openraft snapshot from gossipsub partial sync"
              );
            }
            Err(err) => {
              tracing::warn!(
                peer = %peer_id,
                group = %raft_group_id,
                snapshot_id = %snapshot_id,
                error = ?err,
                "failed to install openraft snapshot from gossipsub partial sync"
              );
            }
          }
        });
      } else {
        tracing::debug!(
          peer = %peer_id,
          group = %update.partial.raft_group_id,
          snapshot_id = %update.partial.snapshot_id,
          partial_group = %group_id_string(&update.partial.group_id),
          parts = update.partial.present_parts(),
          total_parts = update.partial.total_parts(),
          "received openraft snapshot partial"
        );
      }
    }
    gossipsub::Event::Subscribed {
      peer_id,
      topic,
      supports_partial,
      ..
    } => {
      if topic != sync_topic_hash() || !supports_partial {
        tracing::debug!("gossipsub subscribed: peer={peer_id} topic={topic}");
        return;
      }

      let known_snapshots = openraft_sync.known_partials();
      for partial in known_snapshots {
        if let Err(err) = swarm
          .behaviour_mut()
          .gossipsub
          .publish_partial(topic.clone(), partial)
        {
          tracing::debug!(
            peer = %peer_id,
            error = ?err,
            "advertise known openraft snapshot partial failed"
          );
        }
      }
    }
    other => {
      tracing::debug!("gossipsub event: {:?}", other);
    }
  }
}

fn handle_ping_event(
  swarm: &mut Swarm<Behaviour>,
  ping_failures: &mut HashMap<ConnectionId, u32>,
  event: ping::Event,
) {
  let ping::Event {
    peer,
    connection,
    result,
  } = event;
  match result {
    Ok(rtt) => {
      ping_failures.remove(&connection);
      tracing::debug!(peer = %peer, rtt = ?rtt, "ping ok");
    }
    Err(err) => {
      let failures = ping_failures.entry(connection).or_insert(0);
      *failures += 1;
      tracing::warn!(peer = %peer, error = ?err, failures = *failures, "ping failed");
      if *failures >= PING_FAILURE_DISCONNECT_THRESHOLD {
        ping_failures.remove(&connection);
        tracing::warn!(
          peer = %peer,
          ?connection,
          "closing unresponsive connection after repeated ping failures"
        );
        swarm.close_connection(connection);
      }
    }
  }
}

fn handle_connection_established(
  swarm: &mut Swarm<Behaviour>,
  pending_connect: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  connected_peers: &mut HashSet<PeerId>,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  peer_id: PeerId,
) {
  connected_peers.insert(peer_id);
  dial_backoff_until.remove(&peer_id);
  finish_pending_connect(pending_connect, peer_id, Ok(()));
  swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
}

fn handle_connection_closed<E: fmt::Display>(
  swarm: &mut Swarm<Behaviour>,
  connected_peers: &mut HashSet<PeerId>,
  peer_id: PeerId,
  num_established: u32,
  cause: Option<E>,
) {
  if num_established == 0 {
    connected_peers.remove(&peer_id);
    swarm
      .behaviour_mut()
      .gossipsub
      .remove_explicit_peer(&peer_id);
    if let Some(cause) = cause {
      tracing::warn!(peer = %peer_id, error = %cause, "connection closed");
    } else {
      tracing::info!(peer = %peer_id, "connection closed");
    }
  }
}

fn handle_outgoing_connection_error<E: fmt::Display>(
  swarm: &mut Swarm<Behaviour>,
  pending_connect: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  failure_log_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  peer_id: Option<PeerId>,
  error: E,
) {
  let Some(peer_id) = peer_id else {
    tracing::warn!(
      error = %error,
      file = file!(),
      line = line!(),
      "outgoing connection failed"
    );
    return;
  };

  if swarm.is_connected(&peer_id) {
    return;
  }

  let has_waiters = pending_connect.contains_key(&peer_id);
  dial_backoff_until.insert(peer_id, tokio::time::Instant::now() + DIAL_RETRY_BACKOFF);
  if has_waiters && should_log_outgoing_failure(peer_id, failure_log_backoff_until) {
    tracing::warn!(
      peer = %peer_id,
      error = %error,
      file = file!(),
      line = line!(),
      "outgoing connection failed"
    );
  } else {
    tracing::debug!(
      peer = %peer_id,
      error = %error,
      file = file!(),
      line = line!(),
      has_waiters,
      "outgoing connection failed; suppressing warning"
    );
  }
  finish_pending_connect(
    pending_connect,
    peer_id,
    Err(NetErr(format!("dial failed: {error}"))),
  );
}

fn should_log_outgoing_failure(
  peer_id: PeerId,
  failure_log_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
) -> bool {
  let now = tokio::time::Instant::now();
  if let Some(until) = failure_log_backoff_until.get(&peer_id).copied() {
    if now < until {
      return false;
    }
    failure_log_backoff_until.remove(&peer_id);
  }
  failure_log_backoff_until.insert(peer_id, now + OUTGOING_FAILURE_LOG_BACKOFF);
  true
}

/// Client-only swarm loop.
///
/// It supports outbound requests/responses but does not require a `Raft` handle.
/// If it receives an inbound request, it responds with `RaftRpcResponse::Error`.
pub async fn run_swarm_client(swarm: Swarm<Behaviour>, cmd_rx: mpsc::Receiver<Command>) {
  let (_shutdown_tx, shutdown_rx) = crate::signal::channel();
  run_swarm_client_with_shutdown(swarm, cmd_rx, shutdown_rx).await;
}

pub async fn run_swarm_client_with_shutdown(
  mut swarm: Swarm<Behaviour>,
  mut cmd_rx: mpsc::Receiver<Command>,
  mut shutdown_rx: ShutdownRx,
) {
  let mut pending_raft: HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<RaftRpcResponse, NetErr>>,
  > = HashMap::new();
  let mut pending_kv: HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>> =
    HashMap::new();
  let mut pending_sqlite_sync: HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<SqliteSyncRpcResponseMessage, NetErr>>,
  > = HashMap::new();
  let mut pending_task_rpc: HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<TaskRpcResponseMessage, NetErr>>,
  > = HashMap::new();
  let mut pending_connect: HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>> =
    HashMap::new();
  let mut dial_backoff_until: HashMap<PeerId, tokio::time::Instant> = HashMap::new();
  let mut outgoing_failure_log_backoff_until: HashMap<PeerId, tokio::time::Instant> =
    HashMap::new();
  let mut connected_peers: HashSet<PeerId> = HashSet::new();
  let mut pending_start_providing: HashMap<kad::QueryId, oneshot::Sender<Result<(), NetErr>>> =
    HashMap::new();
  let mut pending_get_providers: HashMap<kad::QueryId, GetProvidersState> = HashMap::new();
  let mut ping_failures: HashMap<ConnectionId, u32> = HashMap::new();

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm client");
        leave_openraft_kad(&mut swarm);
        fail_pending_swarm_requests(
          &mut pending_raft,
          &mut pending_kv,
          &mut pending_sqlite_sync,
          &mut pending_task_rpc,
          &mut pending_connect,
          &mut pending_start_providing,
          &mut pending_get_providers,
          "swarm client shutting down",
        );
        break;
      }
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else {
          tracing::warn!("swarm client command channel closed; stopping swarm client");
          leave_openraft_kad(&mut swarm);
          fail_pending_swarm_requests(
            &mut pending_raft,
            &mut pending_kv,
            &mut pending_sqlite_sync,
            &mut pending_task_rpc,
            &mut pending_connect,
            &mut pending_start_providing,
            &mut pending_get_providers,
            "swarm client command channel closed",
          );
          return;
        };
        handle_command(
          &mut swarm,
          cmd,
          &mut pending_raft,
          &mut pending_kv,
          &mut pending_sqlite_sync,
          &mut pending_task_rpc,
          &mut pending_connect,
          &mut dial_backoff_until,
          &mut pending_start_providing,
          &mut pending_get_providers,
        );
      }

      ev = swarm.next() => {
        let Some(ev) = ev else {
          tracing::warn!("swarm client stream ended; stopping swarm client");
          fail_pending_swarm_requests(
            &mut pending_raft,
            &mut pending_kv,
            &mut pending_sqlite_sync,
            &mut pending_task_rpc,
            &mut pending_connect,
            &mut pending_start_providing,
            &mut pending_get_providers,
            "swarm client stream ended",
          );
          break;
        };
        match ev {
          SwarmEvent::Behaviour(BehaviourEvent::Raft(event)) => match event {
            request_response::Event::Message { message, .. } => match message {
              request_response::Message::Request { channel, .. } => {
                let _ = swarm
                  .behaviour_mut()
                  .raft_rpc
                  .send_response(channel, RaftRpcResponse::Error("client-only".to_string()));
              }
              request_response::Message::Response { request_id, response } => {
                if let Some(tx) = pending_raft.remove(&request_id) {
                  let _ = tx.send(Ok(response));
                }
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              if let Some(tx) = pending_raft.remove(&request_id) {
                let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
              }
            }

            request_response::Event::InboundFailure { .. } => {}
            request_response::Event::ResponseSent { .. } => {}
          },

          SwarmEvent::Behaviour(BehaviourEvent::Kv(event)) => match event {
            request_response::Event::Message { message, .. } => match message {
              request_response::Message::Request { channel, .. } => {
                let _ = swarm
                  .behaviour_mut()
                  .kv_rpc
                  .send_response(channel, kv_error_response("client-only"));
              }
              request_response::Message::Response { request_id, response } => {
                if let Some(tx) = pending_kv.remove(&request_id) {
                  let _ = tx.send(Ok(response));
                }
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              if let Some(tx) = pending_kv.remove(&request_id) {
                let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
              }
            }

            request_response::Event::InboundFailure { .. } => {}
            request_response::Event::ResponseSent { .. } => {}
          },

          SwarmEvent::Behaviour(BehaviourEvent::SqliteSync(event)) => match event {
            request_response::Event::Message { message, .. } => match message {
              request_response::Message::Request {
                request, channel, ..
              } => {
                let response = crate::sqlite_cache::process_sqlite_sync_rpc_request(request).await;
                let _ = swarm
                  .behaviour_mut()
                  .sqlite_sync_rpc
                  .send_response(channel, response);
              }
              request_response::Message::Response { request_id, response } => {
                if let Some(tx) = pending_sqlite_sync.remove(&request_id) {
                  let _ = tx.send(Ok(response));
                }
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              if let Some(tx) = pending_sqlite_sync.remove(&request_id) {
                let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
              }
            }

            request_response::Event::InboundFailure { .. } => {}
            request_response::Event::ResponseSent { .. } => {}
          },

          SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => match event {
            mdns::Event::Discovered(list) => {
              // New routing-table entries trigger kad's automatic bootstrap.
              for (peer, addr) in list {
                add_kad_peer_address(&mut swarm, peer, addr);
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
              }
            }
            mdns::Event::Expired(list) => {
              for (peer, addr) in list {
                let addr = strip_p2p(addr);
                swarm.behaviour_mut().kad.remove_address(&peer, &addr);
                swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer);
              }
            }
          },

          SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(event)) => {
            tracing::debug!("gossipsub event: {:?}", event);
          }

          SwarmEvent::Behaviour(BehaviourEvent::Ping(event)) => {
            handle_ping_event(&mut swarm, &mut ping_failures, event);
          }

          SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
            handle_kad_event(&mut swarm, None, None, event, &mut pending_start_providing, &mut pending_get_providers);
          }

          SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            connected_peers.insert(peer_id);
            dial_backoff_until.remove(&peer_id);
            outgoing_failure_log_backoff_until.remove(&peer_id);
            finish_pending_connect(&mut pending_connect, peer_id, Ok(()));
            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
          }

          SwarmEvent::ConnectionClosed { peer_id, connection_id, num_established, .. } => {
            ping_failures.remove(&connection_id);
            if num_established == 0 {
              connected_peers.remove(&peer_id);
              swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
            }
          }

          SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
            handle_outgoing_connection_error(
              &mut swarm,
              &mut pending_connect,
              &mut dial_backoff_until,
              &mut outgoing_failure_log_backoff_until,
              peer_id,
              error,
            );
          }

          SwarmEvent::NewListenAddr { address, .. } => {
            tracing::info!("listening on {address}");
          }

          _ => {}
        }
      }
    }
  }
}

fn ensure_peer_connection(
  swarm: &mut Swarm<Behaviour>,
  pending_connect: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
  peer: PeerId,
  addr: Option<Multiaddr>,
  resp: oneshot::Sender<Result<(), NetErr>>,
) {
  if peer == *swarm.local_peer_id() {
    let _ = resp.send(Err(NetErr(format!("self dial blocked: peer={peer}"))));
    return;
  }

  if swarm.is_connected(&peer) {
    let _ = resp.send(Ok(()));
    return;
  }

  if let Some(until) = dial_backoff_until.get(&peer).copied() {
    let now = tokio::time::Instant::now();
    if now < until {
      let wait_ms = (until - now).as_millis();
      let _ = resp.send(Err(NetErr(format!(
        "dial backoff active: peer={peer}, retry_in_ms={wait_ms}"
      ))));
      return;
    }
    dial_backoff_until.remove(&peer);
  }

  let should_dial = match pending_connect.get_mut(&peer) {
    Some(waiters) => {
      waiters.push(resp);
      false
    }
    None => {
      pending_connect.insert(peer, vec![resp]);
      true
    }
  };

  if let Some(addr) = addr.as_ref() {
    add_kad_address_from_p2p(swarm, addr);
  }
  if should_dial {
    if let Some(addr) = addr {
      dial_known_peer(swarm, peer, addr);
    } else {
      dial_known_peer_any_addr(swarm, peer);
    }
  }
}

fn finish_pending_connect(
  pending_connect: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  peer: PeerId,
  result: Result<(), NetErr>,
) {
  let Some(waiters) = pending_connect.remove(&peer) else {
    return;
  };
  for waiter in waiters {
    let _ = waiter.send(result.clone());
  }
}

fn dial_known_peer(swarm: &mut Swarm<Behaviour>, peer: PeerId, addr: Multiaddr) {
  let dial_opts = DialOpts::peer_id(peer)
    .condition(PeerCondition::DisconnectedAndNotDialing)
    .addresses(vec![addr])
    .build();
  let _ = Swarm::dial(swarm, dial_opts);
}

fn dial_known_peer_any_addr(swarm: &mut Swarm<Behaviour>, peer: PeerId) {
  let dial_opts = DialOpts::peer_id(peer)
    .condition(PeerCondition::DisconnectedAndNotDialing)
    .build();
  let _ = Swarm::dial(swarm, dial_opts);
}

fn dial_peer_addr(swarm: &mut Swarm<Behaviour>, addr: Multiaddr) {
  let peer = addr.iter().last().and_then(|protocol| match protocol {
    libp2p::multiaddr::Protocol::P2p(peer) => Some(peer),
    _ => None,
  });
  if let Some(peer) = peer {
    if peer == *swarm.local_peer_id() {
      tracing::debug!(peer = %peer, addr = %addr, "skip self dial");
      return;
    }
    dial_known_peer(swarm, peer, addr);
  } else {
    let _ = Swarm::dial(swarm, addr);
  }
}

fn add_kad_address_from_p2p(swarm: &mut Swarm<Behaviour>, addr: &Multiaddr) {
  let mut addr = addr.clone();
  let Some(libp2p::multiaddr::Protocol::P2p(peer)) = addr.pop() else {
    return;
  };
  add_kad_peer_address(swarm, peer, addr);
}

fn add_kad_peer_address(swarm: &mut Swarm<Behaviour>, peer: PeerId, addr: Multiaddr) {
  let addr = strip_p2p(addr);
  swarm.behaviour_mut().kad.add_address(&peer, addr);
}

fn strip_p2p(mut addr: Multiaddr) -> Multiaddr {
  if matches!(
    addr.iter().last(),
    Some(libp2p::multiaddr::Protocol::P2p(_))
  ) {
    let _ = addr.pop();
  }
  addr
}

fn leave_openraft_kad(swarm: &mut Swarm<Behaviour>) {
  leave_kad(swarm, &[OPENRAFT_CLUSTER_PROVIDER_KEY.to_string()]);
}

fn leave_kad(swarm: &mut Swarm<Behaviour>, provider_keys: &[String]) {
  for key in provider_keys {
    let record_key = kad::RecordKey::new(key);
    swarm.behaviour_mut().kad.stop_providing(&record_key);
  }
  swarm.behaviour_mut().kad.set_mode(Some(kad::Mode::Client));
  tracing::info!(provider_keys = ?provider_keys, "left kademlia provider mode");
}

fn fail_pending_swarm_requests(
  pending_raft: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftRpcResponse, NetErr>>>,
  pending_kv: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>>,
  pending_sqlite_sync: &mut HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<SqliteSyncRpcResponseMessage, NetErr>>,
  >,
  pending_task_rpc: &mut HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<TaskRpcResponseMessage, NetErr>>,
  >,
  pending_connect: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  pending_start_providing: &mut HashMap<kad::QueryId, oneshot::Sender<Result<(), NetErr>>>,
  pending_get_providers: &mut HashMap<kad::QueryId, GetProvidersState>,
  reason: &'static str,
) {
  fail_pending_map(pending_raft, reason);
  fail_pending_map(pending_kv, reason);
  fail_pending_map(pending_sqlite_sync, reason);
  fail_pending_map(pending_task_rpc, reason);
  fail_pending_map(pending_start_providing, reason);
  fail_pending_connect(pending_connect, reason);
  fail_pending_get_providers(pending_get_providers, reason);
}

fn fail_pending_map<K, T>(
  pending: &mut HashMap<K, oneshot::Sender<Result<T, NetErr>>>,
  reason: &'static str,
) where
  K: Eq + Hash,
{
  for (_, resp) in pending.drain() {
    let _ = resp.send(Err(NetErr(reason.to_string())));
  }
}

fn fail_pending_connect(
  pending: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  reason: &'static str,
) {
  for (_, responses) in pending.drain() {
    for resp in responses {
      let _ = resp.send(Err(NetErr(reason.to_string())));
    }
  }
}

fn fail_pending_get_providers(
  pending: &mut HashMap<kad::QueryId, GetProvidersState>,
  reason: &'static str,
) {
  for (_, state) in pending.drain() {
    let _ = state.resp.send(Err(NetErr(reason.to_string())));
  }
}

fn handle_kad_event(
  swarm: &mut Swarm<Behaviour>,
  network: Option<&Libp2pNetworkFactory>,
  connected_peers: Option<&HashSet<PeerId>>,
  event: kad::Event,
  pending_start_providing: &mut HashMap<kad::QueryId, oneshot::Sender<Result<(), NetErr>>>,
  pending_get_providers: &mut HashMap<kad::QueryId, GetProvidersState>,
) {
  match event {
    kad::Event::RoutingUpdated {
      peer, addresses, ..
    } => {
      if peer == *swarm.local_peer_id() {
        return;
      }
      if let Some(network) = network {
        let addrs: Vec<Multiaddr> = addresses.iter().cloned().collect();
        let network = network.clone();
        tokio::spawn(async move {
          for addr in addrs {
            let _ = network.register_discovered_peer(peer, addr).await;
          }
        });
      }
      if connected_peers.is_none() {
        tracing::debug!(peer = %peer, "kad routing updated (client)");
        return;
      }
      tracing::debug!(
        peer = %peer,
        addresses = ?addresses,
        "kad routing updated"
      );
      if connected_peers.is_some_and(|peers| peers.contains(&peer)) {
        return;
      }
    }
    kad::Event::OutboundQueryProgressed { id, result, .. } => match result {
      kad::QueryResult::GetClosestPeers(result) => match result {
        Ok(ok) => {
          if ok.peers.is_empty() {
            tracing::debug!("kad get_closest_peers complete: no peers");
          } else {
            tracing::debug!(peers = ?ok.peers, "kad get_closest_peers complete");
          }
        }
        Err(err) => {
          tracing::debug!(error = ?err, "kad get_closest_peers failed");
        }
      },
      kad::QueryResult::StartProviding(result) => {
        if let Some(resp) = pending_start_providing.remove(&id) {
          match result {
            Ok(_) => {
              let _ = resp.send(Ok(()));
            }
            Err(e) => {
              let _ = resp.send(Err(NetErr(format!(
                "kademlia start_providing failed: {:?}",
                e
              ))));
            }
          }
        }
      }
      kad::QueryResult::GetProviders(result) => match result {
        Ok(kad::GetProvidersOk::FoundProviders { key: _, providers }) => {
          if let Some(state) = pending_get_providers.get_mut(&id) {
            state.providers.extend(providers);
          }
        }
        Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. }) => {
          if let Some(state) = pending_get_providers.remove(&id) {
            let _ = state.resp.send(Ok(state.providers));
          }
        }
        Err(e) => {
          if let Some(state) = pending_get_providers.remove(&id) {
            let _ = state.resp.send(Err(NetErr(format!(
              "kademlia get_providers failed: {:?}",
              e
            ))));
          }
        }
      },
      _ => {}
    },
    other => {
      tracing::debug!("kad event: {:?}", other);
    }
  }
}

fn kv_error_response(message: impl Into<String>) -> RaftKvResponse {
  RaftKvResponse {
    op: Some(KvResponseOp::Error(ErrorResponse {
      message: message.into(),
      leader_id: String::new(),
    })),
  }
}
