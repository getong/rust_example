use std::{future::Future, time::Duration};

use openraft::{
  AnyError, BasicNode, OptionalSend, RaftNetworkFactory, Snapshot, Vote,
  error::{NetworkError, RPCError, RemoteError, ReplicationClosed, StreamingError},
  network::{Backoff, RPCOption, v2::RaftNetworkV2},
  raft::{
    AppendEntriesRequest, AppendEntriesResponse, SnapshotResponse, VoteRequest, VoteResponse,
  },
};
use serde::{Serialize, de::DeserializeOwned};

use crate::node::{NodeId, RaftTypeConfig};

// Helper struct for sending snapshot data
#[derive(Serialize)]
pub struct SnapshotRequest {
  pub vote: Vote<RaftTypeConfig>,
  pub meta: openraft::SnapshotMeta<RaftTypeConfig>,
  pub data: Vec<u8>,
}

pub struct RaftNetworkClient;

impl RaftNetworkClient {
  pub async fn send_rpc<Req, Resp, Err>(
    &self,
    target: NodeId,
    target_node: &BasicNode,
    uri: &str,
    req: Req,
  ) -> Result<Resp, RPCError<RaftTypeConfig, Err>>
  where
    Req: Serialize,
    Err: std::error::Error + DeserializeOwned,
    Resp: DeserializeOwned,
  {
    let addr = &target_node.addr;
    let url = format!("http://{addr}/{uri}");
    let client = reqwest::Client::new();

    let resp = client
      .post(url)
      .json(&req)
      .send()
      .await
      .map_err(|e| RPCError::Network(NetworkError::new(&e)))?;

    let res: Result<Resp, Err> = resp
      .json()
      .await
      .map_err(|e| RPCError::Network(NetworkError::new(&e)))?;

    res.map_err(|e| RPCError::RemoteError(RemoteError::new(target, e)))
  }
}

// NOTE: This could be implemented also on `Arc<ExampleNetwork>`, but since it's empty, implemented
// directly.
impl RaftNetworkFactory<RaftTypeConfig> for RaftNetworkClient {
  type Network = RaftNetworkConnection;

  async fn new_client(&mut self, target: NodeId, node: &BasicNode) -> Self::Network {
    RaftNetworkConnection {
      owner: RaftNetworkClient {},
      target,
      target_node: node.clone(),
    }
  }
}

pub struct RaftNetworkConnection {
  owner: RaftNetworkClient,
  target: NodeId,
  target_node: BasicNode,
}

impl RaftNetworkV2<RaftTypeConfig> for RaftNetworkConnection {
  async fn append_entries(
    &mut self,
    req: AppendEntriesRequest<RaftTypeConfig>,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse<RaftTypeConfig>, RPCError<RaftTypeConfig>> {
    self
      .owner
      .send_rpc(self.target, &self.target_node, "raft-append", req)
      .await
  }

  async fn vote(
    &mut self,
    req: VoteRequest<RaftTypeConfig>,
    _option: RPCOption,
  ) -> Result<VoteResponse<RaftTypeConfig>, RPCError<RaftTypeConfig>> {
    self
      .owner
      .send_rpc(self.target, &self.target_node, "raft-vote", req)
      .await
  }

  async fn full_snapshot(
    &mut self,
    vote: Vote<RaftTypeConfig>,
    snapshot: Snapshot<RaftTypeConfig>,
    _cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
    _option: RPCOption,
  ) -> Result<SnapshotResponse<RaftTypeConfig>, StreamingError<RaftTypeConfig>> {
    // Send the snapshot in a simplified way - in production you'd want to stream it in chunks
    let req = SnapshotRequest {
      vote,
      meta: snapshot.meta,
      data: snapshot.snapshot.into_inner(), // Convert Cursor<Vec<u8>> to Vec<u8>
    };

    match self
      .owner
      .send_rpc::<_, SnapshotResponse<RaftTypeConfig>, AnyError>(
        self.target,
        &self.target_node,
        "raft-snapshot",
        req,
      )
      .await
    {
      Ok(response) => Ok(response),
      Err(rpc_error) => {
        // Convert RPCError to StreamingError
        match rpc_error {
          RPCError::Network(net_err) => Err(StreamingError::Network(net_err)),
          RPCError::Unreachable(unreachable) => Err(StreamingError::Unreachable(unreachable)),
          RPCError::Timeout(timeout) => Err(StreamingError::Timeout(timeout)),
          RPCError::RemoteError(remote_err) => {
            // Convert to a network error since StreamingError doesn't have RemoteError variant
            Err(StreamingError::Network(NetworkError::new(
              &AnyError::error(format!("Remote error: {:?}", remote_err)),
            )))
          }
        }
      }
    }
  }

  fn backoff(&self) -> Backoff {
    // Return a backoff strategy - exponential backoff starting at 100ms, max 5 seconds
    let backoff_iter = (0 .. 10)
      .map(|i| Duration::from_millis(100 * 2_u64.pow(i.min(6))))
      .chain(std::iter::repeat(Duration::from_secs(5)));

    Backoff::new(backoff_iter)
  }
}
