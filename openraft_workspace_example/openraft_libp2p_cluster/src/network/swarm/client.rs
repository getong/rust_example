//! Typed client handles over the swarm command channel.

use std::{collections::HashSet, time::Duration};

use libp2p::{Multiaddr, PeerId};
use tokio::sync::{mpsc, oneshot};

use super::{
  NetErr, OPENRAFT_CLUSTER_PROVIDER_KEY,
  commands::{Command, Libp2pSwarmReport},
};
use crate::{
  Unreachable,
  network::rpc::{RaftRpcRequest, RaftRpcResponse, UnifiedRpcRequest, UnifiedRpcResponse},
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
  sqlite_sync_rpc::{SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage},
  tasks::rpc::{TaskRpcRequestMessage, TaskRpcResponseMessage},
};

pub(crate) const OPENRAFT_SNAPSHOT_SYNC_TIMEOUT: Duration = Duration::from_secs(45);

/// Priority-separated command senders into the swarm loop.
///
/// Raft traffic (vote, append-entries, snapshot install) rides the `high`
/// channel; KV / sqlite-sync / task RPC and their connection management ride
/// `low`. The loop drains `high` first (`tokio::select!` with `biased`), so
/// a KV burst can no longer queue ahead of raft heartbeats on a shared
/// channel and trigger spurious leader elections (head-of-line blocking).
#[derive(Clone)]
pub struct CommandSenders {
  pub high: mpsc::Sender<Command>,
  pub low: mpsc::Sender<Command>,
}

impl CommandSenders {
  /// Route an inbound-dispatch response back to the loop with the same
  /// priority its request class has.
  pub(crate) fn for_response(&self, resp: &UnifiedRpcResponse) -> &mpsc::Sender<Command> {
    match resp {
      UnifiedRpcResponse::Raft(_) => &self.high,
      _ => &self.low,
    }
  }
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

  pub async fn set_kad_mode(&self, mode: libp2p::kad::Mode) {
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

  /// Build the snapshot partial on the caller's task and hand the finished
  /// partial to the swarm loop for publishing. Building reads the raft
  /// snapshot (disk + serialization) and can take seconds, so it must happen
  /// here — off the swarm loop — and not behind the command channel.
  pub async fn publish_openraft_snapshot(
    &self,
    group_id: String,
    registry: &crate::GroupRegistry,
  ) -> Result<String, NetErr> {
    let partial = match crate::network::openraft_sync::OpenRaftSnapshotPartial::from_raft_group(
      &group_id, registry,
    )
    .await
    {
      Ok(Some(partial)) => partial,
      Ok(None) => {
        return Err(NetErr(format!("openraft group {group_id} has no snapshot")));
      }
      Err(err) => {
        return Err(NetErr(format!(
          "build openraft snapshot partial failed: {err}"
        )));
      }
    };

    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::PublishOpenRaftSnapshotBuilt {
        partial,
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
      .send(Command::RpcRequest {
        peer,
        req: UnifiedRpcRequest::Raft(req),
        resp: resp_tx,
      })
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&NetErr(format!("request timeout: {e}"))))
      .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("response dropped: {e}")))))?;

    match resp.map_err(|e| Unreachable::new(&e))? {
      UnifiedRpcResponse::Raft(resp) => Ok(resp),
      other => Err(Unreachable::new(&NetErr(format!(
        "unexpected rpc response kind: {other:?}"
      )))),
    }
  }
}

/// Generates a typed RPC client over the swarm command channel.
///
/// Every RPC kind needs the same client surface — `connect`, `connect_any`,
/// and a `request` that wraps the kind's `UnifiedRpcRequest` variant and
/// unwraps the matching response variant with a timeout. Only the variant
/// and the request/response types differ, so the ~80 lines per client are
/// stamped out here instead of hand-copied per kind.
macro_rules! define_rpc_client {
  (
    $(#[$meta:meta])*
    $name:ident, $request_variant:ident, $req:ty, $resp:ty
  ) => {
    $(#[$meta])*
    #[derive(Clone)]
    pub struct $name {
      tx: mpsc::Sender<Command>,
      timeout: Duration,
    }

    impl $name {
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

      pub async fn request(&self, peer: PeerId, req: $req) -> Result<$resp, Unreachable> {
        let (resp_tx, resp_rx) = oneshot::channel();
        self
          .tx
          .send(Command::RpcRequest {
            peer,
            req: UnifiedRpcRequest::$request_variant(req),
            resp: resp_tx,
          })
          .await
          .map_err(|e| Unreachable::new(&NetErr(format!("command channel closed: {e}"))))?;

        let resp = tokio::time::timeout(self.timeout, resp_rx)
          .await
          .map_err(|e| Unreachable::new(&NetErr(format!("request timeout: {e}"))))
          .and_then(|r| r.map_err(|e| Unreachable::new(&NetErr(format!("response dropped: {e}")))))?;

        match resp.map_err(|e| Unreachable::new(&e))? {
          UnifiedRpcResponse::$request_variant(resp) => Ok(resp),
          other => Err(Unreachable::new(&NetErr(format!(
            "unexpected rpc response kind: {other:?}"
          )))),
        }
      }
    }
  };
}

define_rpc_client!(KvClient, Kv, RaftKvRequest, RaftKvResponse);
define_rpc_client!(
  SqliteSyncClient,
  SqliteSync,
  SqliteSyncRpcRequestMessage,
  SqliteSyncRpcResponseMessage
);
define_rpc_client!(
  /// Client handle for the tarpc-based task RPC kind of `/openraft/rpc/2`.
  TaskRpcClient,
  Task,
  TaskRpcRequestMessage,
  TaskRpcResponseMessage
);

impl KvClient {
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
}
