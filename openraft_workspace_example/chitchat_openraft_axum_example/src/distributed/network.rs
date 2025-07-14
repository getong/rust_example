use std::collections::BTreeSet;

use openraft::{
  BasicNode,
  error::{InstallSnapshotError, RPCError, RaftError},
  network::{RPCOption, RaftNetwork, RaftNetworkFactory},
  raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
  },
};

use super::raft_types::{NodeId, TypeConfig};

/// Simple network implementation for demo purposes
#[derive(Clone, Default)]
pub struct ChitchatRaftNetwork;

impl ChitchatRaftNetwork {
  pub fn new() -> Self {
    Self
  }

  /// Add a node to the network (for compatibility)
  pub async fn add_node(&self, _node_id: NodeId) {
    // In a real implementation, this would manage network connections
  }

  /// Remove a node from the network (for compatibility)
  pub async fn remove_node(&self, _node_id: &NodeId) {
    // In a real implementation, this would close network connections
  }

  /// Get all connected node IDs (for compatibility)
  pub async fn get_nodes(&self) -> BTreeSet<NodeId> {
    // In a real implementation, this would return actual connected nodes
    BTreeSet::new()
  }
}

impl RaftNetworkFactory<TypeConfig> for ChitchatRaftNetwork {
  type Network = ChitchatRaftNetworkConnection;

  async fn new_client(&mut self, _target: NodeId, _node: &BasicNode) -> Self::Network {
    ChitchatRaftNetworkConnection
  }
}

/// Simple connection implementation for demo purposes
pub struct ChitchatRaftNetworkConnection;

impl RaftNetwork<TypeConfig> for ChitchatRaftNetworkConnection {
  async fn append_entries(
    &mut self,
    _req: AppendEntriesRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<AppendEntriesResponse<TypeConfig>, RPCError<TypeConfig, RaftError<TypeConfig>>> {
    // For demo purposes, just return success
    Ok(AppendEntriesResponse::Success)
  }

  async fn install_snapshot(
    &mut self,
    req: InstallSnapshotRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<
    InstallSnapshotResponse<TypeConfig>,
    RPCError<TypeConfig, RaftError<TypeConfig, InstallSnapshotError>>,
  > {
    // For demo purposes, just return success
    Ok(InstallSnapshotResponse { vote: req.vote })
  }

  async fn vote(
    &mut self,
    req: VoteRequest<TypeConfig>,
    _option: RPCOption,
  ) -> Result<VoteResponse<TypeConfig>, RPCError<TypeConfig, RaftError<TypeConfig>>> {
    // For demo purposes, just return success
    Ok(VoteResponse {
      vote: req.vote,
      vote_granted: true,
      last_log_id: None,
    })
  }
}
