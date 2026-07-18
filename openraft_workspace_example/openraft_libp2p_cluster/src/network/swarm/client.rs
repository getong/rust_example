//! Typed client handles over the swarm command channel.

use std::{collections::HashSet, time::Duration};

use libp2p::{Multiaddr, PeerId};
use tokio::sync::{mpsc, oneshot};

use super::{
  ClusterError, OPENRAFT_CLUSTER_PROVIDER_KEY,
  commands::{Command, Libp2pSwarmReport},
};
use crate::{
  Unreachable,
  network::rpc::{RaftRpcRequest, RaftRpcResponse, UnifiedRpcRequest, UnifiedRpcResponse},
  proto::raft_kv::{RaftKvRequest, RaftKvResponse},
  sqlite_sync_rpc::{SqliteSyncRpcRequestMessage, SqliteSyncRpcResponseMessage},
  tasks::rpc::{TaskRpcRequestMessage, TaskRpcResponseMessage},
  wasm_sync::{WasmSyncRequest, WasmSyncResponse},
};

pub(crate) const OPENRAFT_SNAPSHOT_SYNC_TIMEOUT: Duration = Duration::from_secs(45);

/// Caller-side timeout for latency-critical raft core RPCs (append-entries,
/// vote). Openraft retries these on its own cadence, so a slow follower
/// should fail fast and free its bulkhead/global-limit slot instead of
/// holding it for the general-purpose timeout: at the default 5s, one dead
/// peer can pin `32 slots x 5s` of outbound capacity per timeout window.
/// 2s is several election timeouts — a healthy round-trip is milliseconds.
const RAFT_CORE_RPC_TIMEOUT: Duration = Duration::from_secs(2);

/// Caller-side timeout for full-snapshot install RPCs, which move the whole
/// serialized state machine in one request and are legitimately 10-100x
/// slower than any other RPC. Must stay below the libp2p substream timeout
/// (`RPC_PROTOCOL_REQUEST_TIMEOUT`, 60s) so the caller observes a typed
/// timeout instead of a substream reset.
const RAFT_SNAPSHOT_RPC_TIMEOUT: Duration = Duration::from_secs(45);

/// Tiered per-request timeout (see sixth review §3.5.6): raft core RPCs
/// fail fast, snapshot installs get the long window, and everything else
/// (client-write forwarding, metrics, join/add-learner) keeps the
/// general-purpose `default` configured on the client.
fn raft_rpc_timeout(op: &crate::network::rpc::RaftRpcOp, default: Duration) -> Duration {
  use crate::network::rpc::RaftRpcOp;
  match op {
    // `.min(default)` so an operator who configured an even tighter global
    // timeout is not silently overridden.
    RaftRpcOp::AppendEntries(_) | RaftRpcOp::Vote(_) => RAFT_CORE_RPC_TIMEOUT.min(default),
    RaftRpcOp::FullSnapshot { .. } => RAFT_SNAPSHOT_RPC_TIMEOUT.max(default),
    _ => default,
  }
}

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
  pub async fn libp2p_info(&self) -> Result<Libp2pSwarmReport, ClusterError> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::GetLibp2pInfo { resp: resp_tx })
      .await
      .map_err(|e| ClusterError::Network(format!("command channel closed: {e}")))?;

    tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| ClusterError::Network(format!("timeout: {e}")))
      .and_then(|r| r.map_err(|e| ClusterError::Network(format!("libp2p info dropped: {e}"))))
  }

  pub async fn start_providing(&self, key: String) -> Result<(), ClusterError> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::StartProviding { key, resp: resp_tx })
      .await
      .map_err(|e| ClusterError::Network(format!("command channel closed: {e}")))?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| ClusterError::Network(format!("timeout: {e}")))
      .and_then(|r| r.map_err(|e| ClusterError::Network(format!("connect dropped: {e}"))))?;
    resp
  }

  pub async fn leave_openraft_kad(&self) -> Result<(), ClusterError> {
    self
      .leave_kad(vec![OPENRAFT_CLUSTER_PROVIDER_KEY.to_string()])
      .await
  }

  pub async fn leave_kad(&self, provider_keys: Vec<String>) -> Result<(), ClusterError> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::LeaveKad {
        provider_keys,
        resp: resp_tx,
      })
      .await
      .map_err(|e| ClusterError::Network(format!("command channel closed: {e}")))?;

    tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| ClusterError::Network(format!("timeout: {e}")))
      .and_then(|r| r.map_err(|e| ClusterError::Network(format!("leave_kad dropped: {e}"))))?
  }

  pub async fn get_providers(&self, key: String) -> Result<HashSet<PeerId>, ClusterError> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::GetProviders { key, resp: resp_tx })
      .await
      .map_err(|e| ClusterError::Network(format!("command channel closed: {e}")))?;

    let resp = tokio::time::timeout(self.timeout * 5, resp_rx)
      .await
      .map_err(|e| ClusterError::Network(format!("timeout: {e}")))
      .and_then(|r| r.map_err(|e| ClusterError::Network(format!("connect dropped: {e}"))))?;
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
      .map_err(|e| {
        Unreachable::new(&ClusterError::Network(format!(
          "command channel closed: {e}"
        )))
      })?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&ClusterError::Network(format!("connect timeout: {e}"))))
      .and_then(|r| {
        r.map_err(|e| Unreachable::new(&ClusterError::Network(format!("connect dropped: {e}"))))
      })?;

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
      .map_err(|e| {
        Unreachable::new(&ClusterError::Network(format!(
          "command channel closed: {e}"
        )))
      })?;

    let resp = tokio::time::timeout(self.timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&ClusterError::Network(format!("connect timeout: {e}"))))
      .and_then(|r| {
        r.map_err(|e| Unreachable::new(&ClusterError::Network(format!("connect dropped: {e}"))))
      })?;

    resp.map_err(|e| Unreachable::new(&e))
  }

  pub async fn publish_gossipsub(&self, topic: &str, data: Vec<u8>) -> Result<(), ClusterError> {
    self
      .tx
      .send(Command::GossipsubPublish {
        topic: topic.to_string(),
        data,
      })
      .await
      .map_err(|e| ClusterError::Network(format!("command channel closed: {e}")))
  }

  /// Build the snapshot partial on the caller's task and hand the finished
  /// partial to the swarm loop for publishing. Building reads the raft
  /// snapshot (disk + serialization) and can take seconds, so it must happen
  /// here — off the swarm loop — and not behind the command channel.
  pub async fn publish_openraft_snapshot(
    &self,
    group_id: String,
    registry: &crate::GroupRegistry,
  ) -> Result<String, ClusterError> {
    let partial = match crate::network::openraft_sync::OpenRaftSnapshotPartial::from_raft_group(
      &group_id, registry,
    )
    .await
    {
      Ok(Some(partial)) => partial,
      Ok(None) => {
        return Err(ClusterError::Network(format!(
          "openraft group {group_id} has no snapshot"
        )));
      }
      Err(err) => {
        return Err(ClusterError::Network(format!(
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
      .map_err(|e| ClusterError::Network(format!("command channel closed: {e}")))?;

    let timeout = self.timeout.max(OPENRAFT_SNAPSHOT_SYNC_TIMEOUT);
    tokio::time::timeout(timeout, resp_rx)
      .await
      .map_err(|e| ClusterError::Network(format!("openraft snapshot sync timeout: {e}")))?
      .map_err(|e| ClusterError::Network(format!("openraft snapshot sync dropped: {e}")))?
  }

  pub async fn request(
    &self,
    peer: PeerId,
    req: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable> {
    let timeout = raft_rpc_timeout(&req.op, self.timeout);
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::RpcRequest {
        peer,
        req: UnifiedRpcRequest::Raft(req),
        resp: resp_tx,
      })
      .await
      .map_err(|e| {
        Unreachable::new(&ClusterError::Network(format!(
          "command channel closed: {e}"
        )))
      })?;

    let resp = tokio::time::timeout(timeout, resp_rx)
      .await
      .map_err(|e| Unreachable::new(&ClusterError::Network(format!("request timeout: {e}"))))
      .and_then(|r| {
        r.map_err(|e| Unreachable::new(&ClusterError::Network(format!("response dropped: {e}"))))
      })?;

    match resp.map_err(|e| Unreachable::new(&e))? {
      UnifiedRpcResponse::Raft(resp) => Ok(resp),
      other => Err(Unreachable::new(&ClusterError::Network(format!(
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
          .map_err(|e| Unreachable::new(&ClusterError::Network(format!("command channel closed: {e}"))))?;

        let resp = tokio::time::timeout(self.timeout, resp_rx)
          .await
          .map_err(|e| Unreachable::new(&ClusterError::Network(format!("connect timeout: {e}"))))
          .and_then(|r| r.map_err(|e| Unreachable::new(&ClusterError::Network(format!("connect dropped: {e}")))))?;

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
          .map_err(|e| Unreachable::new(&ClusterError::Network(format!("command channel closed: {e}"))))?;

        let resp = tokio::time::timeout(self.timeout, resp_rx)
          .await
          .map_err(|e| Unreachable::new(&ClusterError::Network(format!("connect timeout: {e}"))))
          .and_then(|r| r.map_err(|e| Unreachable::new(&ClusterError::Network(format!("connect dropped: {e}")))))?;

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
          .map_err(|e| Unreachable::new(&ClusterError::Network(format!("command channel closed: {e}"))))?;

        let resp = tokio::time::timeout(self.timeout, resp_rx)
          .await
          .map_err(|e| Unreachable::new(&ClusterError::Network(format!("request timeout: {e}"))))
          .and_then(|r| r.map_err(|e| Unreachable::new(&ClusterError::Network(format!("response dropped: {e}")))))?;

        match resp.map_err(|e| Unreachable::new(&e))? {
          UnifiedRpcResponse::$request_variant(resp) => Ok(resp),
          other => Err(Unreachable::new(&ClusterError::Network(format!(
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
define_rpc_client!(
  /// Client handle for the wasm module sync kind of `/openraft/rpc/2`
  /// (manifest + chunk pulls, see [`crate::wasm_sync`]).
  WasmSyncClient,
  WasmSync,
  WasmSyncRequest,
  WasmSyncResponse
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

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{
    NodeId,
    network::rpc::RaftRpcOp,
    typ::{SnapshotMeta, Vote, VoteRequest},
  };

  #[test]
  fn rpc_timeouts_are_tiered_by_op() {
    let default = Duration::from_secs(5);
    let vote = Vote::new(1, NodeId::from("node-a"));
    let vote_req = || RaftRpcOp::Vote(VoteRequest::new(vote.clone(), None));

    // Raft core RPCs fail fast...
    assert_eq!(
      raft_rpc_timeout(&vote_req(), default),
      RAFT_CORE_RPC_TIMEOUT
    );
    // ...unless the operator configured an even tighter global timeout.
    let tight = Duration::from_secs(1);
    assert_eq!(raft_rpc_timeout(&vote_req(), tight), tight);

    // Snapshot installs get the long window.
    assert_eq!(
      raft_rpc_timeout(
        &RaftRpcOp::FullSnapshot {
          vote,
          meta: SnapshotMeta::default(),
          data: Vec::new(),
        },
        default
      ),
      RAFT_SNAPSHOT_RPC_TIMEOUT
    );

    // Everything else keeps the general-purpose timeout.
    assert_eq!(raft_rpc_timeout(&RaftRpcOp::GetMetrics, default), default);
  }
}
