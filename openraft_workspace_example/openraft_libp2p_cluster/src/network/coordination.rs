//! Transport-independent coordination decisions shared by startup, joining,
//! and leader-side maintenance.
//!
//! OpenRaft exposes a fairly large `RaftMetrics` value. Passing that value
//! through application code caused each caller to re-interpret leader and
//! membership state slightly differently. `MembershipView` is the small
//! coordination contract: it is derived from OpenRaft once, and contains no
//! libp2p types or networking behavior.

use std::collections::BTreeSet;

use crate::{NodeId, typ::RaftMetrics};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MembershipView {
  current_leader: Option<NodeId>,
  leader_addr: Option<String>,
  voters: BTreeSet<NodeId>,
  members: BTreeSet<NodeId>,
  local_is_leader: bool,
}

impl MembershipView {
  pub fn from_metrics(metrics: &RaftMetrics) -> Self {
    let membership = metrics.membership_config.membership();
    let voters = membership.voter_ids().collect();
    let members = membership.nodes().map(|(id, _)| id.clone()).collect();
    let leader_addr = metrics
      .current_leader
      .as_ref()
      .and_then(|leader_id| membership.get_node(leader_id).map(|node| node.addr.clone()));

    Self {
      current_leader: metrics.current_leader.clone(),
      leader_addr,
      voters,
      members,
      local_is_leader: metrics.state.is_leader(),
    }
  }

  pub fn current_leader(&self) -> Option<&NodeId> {
    self.current_leader.as_ref()
  }

  pub fn leader_addr(&self) -> Option<&str> {
    self.leader_addr.as_deref()
  }

  pub fn is_local_leader(&self) -> bool {
    self.local_is_leader
  }

  pub fn is_voter(&self, node_id: &NodeId) -> bool {
    self.voters.contains(node_id)
  }

  pub fn is_member(&self, node_id: &NodeId) -> bool {
    self.members.contains(node_id)
  }

  pub fn voter_count(&self) -> usize {
    self.voters.len()
  }

  pub fn member_count(&self) -> usize {
    self.members.len()
  }

  pub fn voters(&self) -> &BTreeSet<NodeId> {
    &self.voters
  }

  pub fn members(&self) -> &BTreeSet<NodeId> {
    &self.members
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn empty_view_is_conservative() {
    let view = MembershipView::default();
    assert!(view.current_leader().is_none());
    assert!(view.leader_addr().is_none());
    assert!(!view.is_local_leader());
    assert_eq!(view.voter_count(), 0);
    assert_eq!(view.member_count(), 0);
  }
}
