use std::{
  collections::{HashMap, HashSet},
  error::Error,
  fmt,
  sync::Arc,
  time::Duration,
};

use futures::{StreamExt, future::poll_fn};
use libp2p::{
  Multiaddr, PeerId, Swarm, gossipsub,
  kad::{self, store::MemoryStore},
  mdns, ping,
  request_response::{self, OutboundRequestId, ResponseChannel},
  swarm::{
    NetworkBehaviour, SwarmEvent,
    dial_opts::{DialOpts, PeerCondition},
  },
};
use once_cell::sync::OnceCell;
use prost::Message;
use tokio::sync::{Mutex, MutexGuard, mpsc, oneshot};

use crate::{
  Unreachable,
  network::{
    dispatcher::SwarmRequestDispatcher,
    openraft_sync::{OpenRaftSnapshotPartial, OpenRaftSyncState, hex_id, sync_topic_hash},
    proto_codec::{ProstCodec, ProtoCodec, SerdeCodec},
    rpc::{RaftRpcRequest, RaftRpcResponse},
    transport::Libp2pNetworkFactory,
  },
  proto::raft_kv::{
    ChatMessage, ErrorResponse, RaftKvRequest, RaftKvResponse, raft_kv_response::Op as KvResponseOp,
  },
  signal::ShutdownRx,
  sqlite_sync_rpc::{SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage},
};

pub const GOSSIP_TOPIC: &str = "openraft/cluster/1";
const DIAL_RETRY_BACKOFF: Duration = Duration::from_secs(2);
const OPENRAFT_SNAPSHOT_SYNC_TIMEOUT: Duration = Duration::from_secs(45);

pub type SharedSwarm = Arc<Mutex<Swarm<Behaviour>>>;

pub static LIBP2P_SWARM: OnceCell<SharedSwarm> = OnceCell::new();

pub fn set_libp2p_swarm(swarm: SharedSwarm) -> Result<(), SharedSwarm> {
  LIBP2P_SWARM.set(swarm)
}

pub fn libp2p_swarm() -> Option<SharedSwarm> {
  LIBP2P_SWARM.get().cloned()
}

pub async fn lock_swarm(swarm: &SharedSwarm) -> MutexGuard<'_, Swarm<Behaviour>> {
  swarm.lock().await
}

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
  PublishOpenRaftSnapshot {
    group_id: String,
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

pub async fn run_swarm(
  mut cmd_rx: mpsc::Receiver<Command>,
  cmd_tx: mpsc::Sender<Command>,
  network: Libp2pNetworkFactory,
  dispatcher: Arc<dyn SwarmRequestDispatcher>,
  mut shutdown_rx: ShutdownRx,
) {
  let Some(swarm) = libp2p_swarm() else {
    tracing::error!("global libp2p swarm is not initialized");
    return;
  };

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
  let mut pending_connect: HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>> =
    HashMap::new();
  let mut openraft_sync = OpenRaftSyncState::default();
  let mut dial_backoff_until: HashMap<PeerId, tokio::time::Instant> = HashMap::new();
  let mut connected_peers: HashSet<PeerId> = HashSet::new();
  let mut reconnect_tick = tokio::time::interval(Duration::from_secs(12));
  let mut kad_discovery_tick = tokio::time::interval(Duration::from_secs(30));
  reconnect_tick.tick().await;
  kad_discovery_tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm");
        break;
      }
      _ = reconnect_tick.tick() => {
        handle_reconnect_tick(&swarm, &network, &connected_peers).await;
      }
      _ = kad_discovery_tick.tick() => {
        let mut swarm = lock_swarm(&swarm).await;
        kick_kad_queries(&mut swarm);
      }
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { return; };
        if let Command::PublishOpenRaftSnapshot { group_id, resp } = cmd {
          handle_publish_openraft_snapshot(&swarm, group_id, resp, &mut openraft_sync).await;
          continue;
        }

        let mut swarm = lock_swarm(&swarm).await;
        handle_command(
          &mut swarm,
          cmd,
          &mut pending_raft,
          &mut pending_kv,
          &mut pending_sqlite_sync,
          &mut pending_connect,
          &mut dial_backoff_until,
        );
      }

      ev = next_swarm_event(&swarm) => {
        handle_swarm_event(
          &swarm,
          ev,
          &network,
          dispatcher.clone(),
          &cmd_tx,
          &mut pending_raft,
          &mut pending_kv,
          &mut pending_sqlite_sync,
          &mut pending_connect,
          &mut openraft_sync,
          &mut connected_peers,
          &mut dial_backoff_until,
        )
        .await;
      }
    }
  }
}

async fn handle_reconnect_tick(
  swarm: &SharedSwarm,
  network: &Libp2pNetworkFactory,
  connected_peers: &HashSet<PeerId>,
) {
  let nodes = network.known_nodes().await;
  let mut swarm = lock_swarm(swarm).await;
  for (_node_id, peer_id, addr) in nodes {
    if peer_id == *swarm.local_peer_id() {
      continue;
    }
    if connected_peers.contains(&peer_id) {
      continue;
    }
    tracing::info!(
      peer = %peer_id,
      addr = %addr,
      "reconnecting to peer"
    );
    dial_peer_addr(&mut swarm, addr.clone());
    add_kad_address_from_p2p(&mut swarm, &addr);
  }
}

async fn next_swarm_event(swarm: &SharedSwarm) -> SwarmEvent<BehaviourEvent> {
  loop {
    if let Some(event) = poll_fn(|cx| {
      let Ok(mut swarm) = swarm.try_lock() else {
        return std::task::Poll::Ready(None);
      };
      match swarm.poll_next_unpin(cx) {
        std::task::Poll::Ready(Some(event)) => std::task::Poll::Ready(Some(event)),
        std::task::Poll::Ready(None) => panic!("swarm stream ended"),
        std::task::Poll::Pending => std::task::Poll::Pending,
      }
    })
    .await
    {
      return event;
    }

    tokio::task::yield_now().await;
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
  pending_connect: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
) {
  match cmd {
    Command::Dial { addr } => {
      dial_peer_addr(swarm, addr.clone());
      add_kad_address_from_p2p(swarm, &addr);
      kick_kad_queries(swarm);
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
    Command::PublishOpenRaftSnapshot { resp, .. } => {
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
  }
}

async fn handle_publish_openraft_snapshot(
  swarm: &SharedSwarm,
  group_id: String,
  resp: oneshot::Sender<Result<String, NetErr>>,
  openraft_sync: &mut OpenRaftSyncState,
) {
  let local_peer_id = {
    let swarm = lock_swarm(swarm).await;
    *swarm.local_peer_id()
  };

  let partial = match OpenRaftSnapshotPartial::from_raft_group(&group_id, local_peer_id).await {
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

  let sync_group = hex_id(&partial.group_id);
  let topic = sync_topic_hash();
  let publish_result = {
    let mut swarm = lock_swarm(swarm).await;
    swarm
      .behaviour_mut()
      .gossipsub
      .publish_partial(topic, partial.clone())
  };

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
  swarm: &SharedSwarm,
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
  pending_connect: &mut HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>>,
  openraft_sync: &mut OpenRaftSyncState,
  connected_peers: &mut HashSet<PeerId>,
  dial_backoff_until: &mut HashMap<PeerId, tokio::time::Instant>,
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
    SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => {
      handle_mdns_event(swarm, network, event).await;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(event)) => {
      handle_gossipsub_event(swarm, openraft_sync, event).await;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Ping(event)) => {
      handle_ping_event(event);
    }
    SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
      let mut swarm = lock_swarm(swarm).await;
      handle_kad_event(&mut swarm, Some(network), Some(connected_peers), event);
    }
    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
      {
        let mut swarm = lock_swarm(swarm).await;
        handle_connection_established(
          &mut swarm,
          pending_connect,
          connected_peers,
          dial_backoff_until,
          peer_id,
        );
      }
      network.set_peer_connected(peer_id).await;
    }
    SwarmEvent::ConnectionClosed {
      peer_id,
      num_established,
      cause,
      ..
    } => {
      {
        let mut swarm = lock_swarm(swarm).await;
        handle_connection_closed(&mut swarm, connected_peers, peer_id, num_established, cause);
      }
      if num_established == 0 {
        network.set_peer_disconnected(peer_id).await;
      }
    }
    SwarmEvent::NewListenAddr { address, .. } => {
      tracing::info!("listening on {address}");
    }
    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
      let mut swarm = lock_swarm(swarm).await;
      handle_outgoing_connection_error(
        &mut swarm,
        pending_connect,
        dial_backoff_until,
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

async fn handle_mdns_event(
  swarm: &SharedSwarm,
  network: &Libp2pNetworkFactory,
  event: mdns::Event,
) {
  match event {
    mdns::Event::Discovered(list) => {
      let mut saw_peer = false;
      for (peer, addr) in list {
        let mut use_discovered_addr = network.update_peer_addr_from_mdns(peer, addr.clone()).await;
        if network.register_discovered_peer(peer, addr.clone()).await {
          use_discovered_addr = true;
        }
        if use_discovered_addr {
          saw_peer = true;
          let mut swarm = lock_swarm(swarm).await;
          add_kad_peer_address(&mut swarm, peer, addr);
        }
        let mut swarm = lock_swarm(swarm).await;
        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
      }
      if saw_peer {
        let mut swarm = lock_swarm(swarm).await;
        kick_kad_queries(&mut swarm);
      }
    }
    mdns::Event::Expired(list) => {
      let mut swarm = lock_swarm(swarm).await;
      for (peer, addr) in list {
        let addr = strip_p2p(addr);
        swarm.behaviour_mut().kad.remove_address(&peer, &addr);
        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer);
      }
    }
  }
}

async fn handle_gossipsub_event(
  swarm: &SharedSwarm,
  openraft_sync: &mut OpenRaftSyncState,
  event: gossipsub::Event,
) {
  match event {
    gossipsub::Event::Message {
      propagation_source,
      message_id,
      message,
    } => match ChatMessage::decode(message.data.as_slice()) {
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
    },
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
        tracing::warn!(peer = %peer_id, group = %hex_id(&group_id), "openraft snapshot partial missing metadata");
        let mut swarm = lock_swarm(swarm).await;
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
              group = %hex_id(&group_id),
              error = ?err,
              "invalid openraft snapshot partial"
            );
            let mut swarm = lock_swarm(swarm).await;
            swarm
              .behaviour_mut()
              .gossipsub
              .report_invalid_partial(peer_id, &topic_hash);
            return;
          }
        };

      if update.should_republish {
        let mut swarm = lock_swarm(swarm).await;
        if let Err(err) = swarm
          .behaviour_mut()
          .gossipsub
          .publish_partial(topic_hash.clone(), update.partial.clone())
        {
          tracing::debug!(error = ?err, "republish openraft snapshot partial failed");
        }
      }

      if update.first_complete {
        let raft_group_id = update.partial.raft_group_id.clone();
        let snapshot_id = update.partial.snapshot_id.clone();
        match update.partial.install().await {
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
      } else {
        tracing::debug!(
          peer = %peer_id,
          group = %update.partial.raft_group_id,
          snapshot_id = %update.partial.snapshot_id,
          partial_group = %hex_id(&update.partial.group_id),
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
      let mut swarm = lock_swarm(swarm).await;
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

fn handle_ping_event(event: ping::Event) {
  match event {
    ping::Event {
      peer,
      result: Ok(rtt),
      ..
    } => {
      tracing::debug!(peer = %peer, rtt = ?rtt, "ping ok");
    }
    ping::Event {
      peer,
      result: Err(err),
      ..
    } => {
      tracing::warn!(peer = %peer, error = ?err, "ping failed");
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
  kick_kad_queries(swarm);
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
  peer_id: Option<PeerId>,
  error: E,
) {
  let Some(peer_id) = peer_id else {
    tracing::warn!(error = %error, "outgoing connection failed");
    return;
  };

  if swarm.is_connected(&peer_id) {
    return;
  }

  dial_backoff_until.insert(peer_id, tokio::time::Instant::now() + DIAL_RETRY_BACKOFF);
  tracing::warn!(peer = %peer_id, error = %error, "outgoing connection failed");
  finish_pending_connect(
    pending_connect,
    peer_id,
    Err(NetErr(format!("dial failed: {error}"))),
  );
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
  let mut pending_connect: HashMap<PeerId, Vec<oneshot::Sender<Result<(), NetErr>>>> =
    HashMap::new();
  let mut dial_backoff_until: HashMap<PeerId, tokio::time::Instant> = HashMap::new();
  let mut connected_peers: HashSet<PeerId> = HashSet::new();
  let mut kad_discovery_tick = tokio::time::interval(Duration::from_secs(30));
  kad_discovery_tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm client");
        break;
      }
      _ = kad_discovery_tick.tick() => {
        kick_kad_queries(&mut swarm);
      }
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { return; };
        handle_command(
          &mut swarm,
          cmd,
          &mut pending_raft,
          &mut pending_kv,
          &mut pending_sqlite_sync,
          &mut pending_connect,
          &mut dial_backoff_until,
        );
      }

      ev = swarm.select_next_some() => {
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
              let mut saw_peer = false;
              for (peer, addr) in list {
                saw_peer = true;
                add_kad_peer_address(&mut swarm, peer, addr);
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
              }
              if saw_peer {
                kick_kad_queries(&mut swarm);
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
            handle_ping_event(event);
          }

          SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
            handle_kad_event(&mut swarm, None, None, event);
          }

          SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            connected_peers.insert(peer_id);
            dial_backoff_until.remove(&peer_id);
            finish_pending_connect(&mut pending_connect, peer_id, Ok(()));
            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
            kick_kad_queries(&mut swarm);
          }

          SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
            if num_established == 0 {
              connected_peers.remove(&peer_id);
              swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
            }
          }

          SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
            let Some(peer_id) = peer_id else {
              tracing::warn!(error = %error, "outgoing connection failed");
              continue;
            };
            if connected_peers.contains(&peer_id) || swarm.is_connected(&peer_id) {
              continue;
            }
            dial_backoff_until.insert(peer_id, tokio::time::Instant::now() + DIAL_RETRY_BACKOFF);
            tracing::warn!(peer = %peer_id, error = %error, "outgoing connection failed");
            finish_pending_connect(
              &mut pending_connect,
              peer_id,
              Err(NetErr(format!("dial failed: {error}"))),
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
    kick_kad_queries(swarm);
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

fn kick_kad_queries(swarm: &mut Swarm<Behaviour>) {
  let local_peer_id = swarm.local_peer_id().to_owned();
  let _ = swarm.behaviour_mut().kad.bootstrap();
  swarm.behaviour_mut().kad.get_closest_peers(local_peer_id);
}

fn handle_kad_event(
  swarm: &mut Swarm<Behaviour>,
  network: Option<&Libp2pNetworkFactory>,
  connected_peers: Option<&HashSet<PeerId>>,
  event: kad::Event,
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
    kad::Event::OutboundQueryProgressed { result, .. } => {
      if let kad::QueryResult::GetClosestPeers(result) = result {
        match result {
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
        }
      }
    }
    other => {
      tracing::debug!("kad event: {:?}", other);
    }
  }
}

fn kv_error_response(message: impl Into<String>) -> RaftKvResponse {
  RaftKvResponse {
    op: Some(KvResponseOp::Error(ErrorResponse {
      message: message.into(),
    })),
  }
}
