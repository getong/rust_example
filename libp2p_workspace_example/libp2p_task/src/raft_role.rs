use std::sync::Arc;

use openraft::ServerState;
use tokio::sync::RwLock;

#[derive(Clone)]
pub(crate) struct OpenRaftRoleTracker {
  local_node_id: Arc<str>,
  state: Arc<RwLock<Option<ServerState>>>,
  current_leader: Arc<RwLock<Option<String>>>,
}

impl OpenRaftRoleTracker {
  pub(crate) fn new(
    local_node_id: impl Into<String>,
    state: Option<ServerState>,
    current_leader: Option<String>,
  ) -> Self {
    Self {
      local_node_id: Arc::from(local_node_id.into()),
      state: Arc::new(RwLock::new(state)),
      current_leader: Arc::new(RwLock::new(current_leader)),
    }
  }

  pub(crate) fn local_node_id(&self) -> &str {
    &self.local_node_id
  }

  pub(crate) async fn state(&self) -> Option<ServerState> {
    *self.state.read().await
  }

  pub(crate) async fn set_state(&self, state: Option<ServerState>) {
    *self.state.write().await = state;
  }

  pub(crate) async fn current_leader(&self) -> Option<String> {
    self.current_leader.read().await.clone()
  }

  pub(crate) async fn set_current_leader(&self, leader_id: Option<String>) {
    *self.current_leader.write().await = leader_id;
  }

  pub(crate) async fn is_follower(&self) -> bool {
    self.state().await == Some(ServerState::Follower)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn follower_state_can_run_apalis_task() {
    let tracker = OpenRaftRoleTracker::new("node-a", Some(ServerState::Follower), None);

    assert!(tracker.is_follower().await);
  }

  #[tokio::test]
  async fn leader_state_cannot_run_apalis_task() {
    let tracker = OpenRaftRoleTracker::new("node-a", Some(ServerState::Leader), None);

    assert!(!tracker.is_follower().await);
  }

  #[tokio::test]
  async fn unknown_state_cannot_run_apalis_task() {
    let tracker = OpenRaftRoleTracker::new("node-a", None, None);

    assert!(!tracker.is_follower().await);
  }

  #[tokio::test]
  async fn can_update_state_from_openraft_metrics() {
    let tracker = OpenRaftRoleTracker::new("node-a", None, None);

    assert!(!tracker.is_follower().await);

    tracker.set_state(Some(ServerState::Follower)).await;

    assert!(tracker.is_follower().await);
  }

  #[tokio::test]
  async fn can_update_leader_from_openraft_metrics() {
    let tracker = OpenRaftRoleTracker::new("node-a", Some(ServerState::Follower), None);

    tracker.set_current_leader(Some("node-b".to_string())).await;

    assert_eq!(tracker.current_leader().await, Some("node-b".to_string()));
  }
}
