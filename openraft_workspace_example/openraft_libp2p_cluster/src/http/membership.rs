use std::{collections::BTreeSet, sync::Arc, time::Duration};

use axum::extract::State;
use openraft::{BasicNode, async_runtime::WatchReceiver, log_id::RaftLogId};
use serde::{Deserialize, Serialize};

use super::{AppState, Json, openraft_group_ids};
use crate::{
  NodeId,
  network::{
    rpc::{AddLearnerRequest, RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
};

#[derive(Deserialize)]
pub(super) struct RemoveOpenRaftMemberRequest {
  node_id: NodeId,
  group_id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct AddOpenRaftMemberRequest {
  node_id: NodeId,
  addr: Option<String>,
  group_id: Option<String>,
  #[serde(default = "default_promote_openraft_member")]
  promote: bool,
  catch_up_timeout_secs: Option<u64>,
}

fn default_promote_openraft_member() -> bool {
  true
}

#[derive(Serialize)]
pub(super) struct AddOpenRaftMemberResponse {
  ok: bool,
  target_node_id: NodeId,
  groups: Vec<AddOpenRaftMemberGroupResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct AddOpenRaftMemberGroupResponse {
  group_id: String,
  ok: bool,
  before_voters: Vec<NodeId>,
  after_voters: Vec<NodeId>,
  leader_id: Option<NodeId>,
  learner_added: bool,
  promoted: bool,
  error: Option<String>,
}

#[derive(Serialize)]
pub(super) struct RemoveOpenRaftMemberResponse {
  ok: bool,
  target_node_id: NodeId,
  groups: Vec<RemoveOpenRaftMemberGroupResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct RemoveOpenRaftMemberGroupResponse {
  group_id: String,
  ok: bool,
  before_voters: Vec<NodeId>,
  after_voters: Vec<NodeId>,
  leader_id: Option<NodeId>,
  error: Option<String>,
}

/// Resolve the target group set for a membership request: an explicit
/// `group_id`, or every initialized group.
fn resolve_membership_group_ids(state: &AppState, group_id: Option<String>) -> Vec<String> {
  match group_id {
    Some(group_id) => vec![group_id],
    None => openraft_group_ids(&state.registry),
  }
}

pub(super) async fn remove_openraft_member(
  State(state): State<Arc<AppState>>,
  Json(req): Json<RemoveOpenRaftMemberRequest>,
) -> Json<RemoveOpenRaftMemberResponse> {
  let group_ids = resolve_membership_group_ids(&state, req.group_id.clone());
  Json(apply_remove_openraft_member(&state, req.node_id, &group_ids).await)
}

async fn apply_remove_openraft_member(
  state: &AppState,
  node_id: NodeId,
  group_ids: &[String],
) -> RemoveOpenRaftMemberResponse {
  if group_ids.is_empty() {
    return RemoveOpenRaftMemberResponse {
      ok: false,
      target_node_id: node_id,
      groups: Vec::new(),
      error: Some("openraft groups are not initialized".to_string()),
    };
  }

  let mut groups = Vec::with_capacity(group_ids.len());
  for group_id in group_ids {
    groups.push(remove_openraft_member_from_group(&state.registry, group_id, &node_id).await);
  }
  let ok = groups.iter().all(|group| group.ok);
  let error = if ok {
    None
  } else {
    Some("one or more openraft membership changes failed".to_string())
  };

  RemoveOpenRaftMemberResponse {
    ok,
    target_node_id: node_id,
    groups,
    error,
  }
}

pub(super) async fn add_openraft_member(
  State(state): State<Arc<AppState>>,
  Json(req): Json<AddOpenRaftMemberRequest>,
) -> Json<AddOpenRaftMemberResponse> {
  let group_ids = resolve_membership_group_ids(&state, req.group_id.clone());
  let catch_up_timeout = Duration::from_secs(req.catch_up_timeout_secs.unwrap_or(30));
  Json(
    apply_add_openraft_member(
      &state,
      req.node_id,
      req.addr,
      &group_ids,
      req.promote,
      catch_up_timeout,
    )
    .await,
  )
}

async fn apply_add_openraft_member(
  state: &AppState,
  node_id: NodeId,
  addr: Option<String>,
  group_ids: &[String],
  promote: bool,
  catch_up_timeout: Duration,
) -> AddOpenRaftMemberResponse {
  if group_ids.is_empty() {
    return AddOpenRaftMemberResponse {
      ok: false,
      target_node_id: node_id,
      groups: Vec::new(),
      error: Some("openraft groups are not initialized".to_string()),
    };
  }

  let target_addr = match resolve_openraft_member_addr(state, &node_id, addr).await {
    Ok(addr) => addr,
    Err(err) => {
      return AddOpenRaftMemberResponse {
        ok: false,
        target_node_id: node_id,
        groups: Vec::new(),
        error: Some(err),
      };
    }
  };

  if let Err(err) = state
    .network
    .register_node(node_id.clone(), &target_addr)
    .await
  {
    return AddOpenRaftMemberResponse {
      ok: false,
      target_node_id: node_id,
      groups: Vec::new(),
      error: Some(format!("register target node failed: {err}")),
    };
  }

  let mut groups = Vec::with_capacity(group_ids.len());
  for group_id in group_ids {
    groups.push(
      add_openraft_member_to_group(
        &state.registry,
        group_id,
        &node_id,
        &target_addr,
        promote,
        catch_up_timeout,
        &state.network,
      )
      .await,
    );
  }
  let ok = groups.iter().all(|group| group.ok);
  let error = if ok {
    None
  } else {
    Some("one or more openraft membership changes failed".to_string())
  };

  AddOpenRaftMemberResponse {
    ok,
    target_node_id: node_id,
    groups,
    error,
  }
}

#[derive(Deserialize)]
pub(super) struct BatchMembershipRequest {
  /// Members to add (learner + optional promote), applied before removals.
  #[serde(default)]
  add: Vec<BatchAddMember>,
  /// Voters to remove after the additions completed.
  #[serde(default)]
  remove: Vec<NodeId>,
  group_id: Option<String>,
  #[serde(default = "default_promote_openraft_member")]
  promote: bool,
  catch_up_timeout_secs: Option<u64>,
}

#[derive(Deserialize)]
pub(super) struct BatchAddMember {
  node_id: NodeId,
  addr: Option<String>,
}

#[derive(Serialize)]
pub(super) struct BatchMembershipResponse {
  ok: bool,
  added: Vec<AddOpenRaftMemberResponse>,
  removed: Vec<RemoveOpenRaftMemberResponse>,
  error: Option<String>,
}

/// Apply several membership changes in one call: all additions first (so a
/// scale-out plus scale-in swap never passes through an under-replicated
/// intermediate state), then all removals. Each change reports its own
/// per-group result; `ok` is the conjunction.
pub(super) async fn batch_openraft_membership(
  State(state): State<Arc<AppState>>,
  Json(req): Json<BatchMembershipRequest>,
) -> Json<BatchMembershipResponse> {
  let group_ids = resolve_membership_group_ids(&state, req.group_id.clone());
  let catch_up_timeout = Duration::from_secs(req.catch_up_timeout_secs.unwrap_or(30));

  let mut added = Vec::with_capacity(req.add.len());
  for member in req.add {
    added.push(
      apply_add_openraft_member(
        &state,
        member.node_id,
        member.addr,
        &group_ids,
        req.promote,
        catch_up_timeout,
      )
      .await,
    );
  }

  let mut removed = Vec::with_capacity(req.remove.len());
  for node_id in req.remove {
    removed.push(apply_remove_openraft_member(&state, node_id, &group_ids).await);
  }

  let ok = added.iter().all(|result| result.ok) && removed.iter().all(|result| result.ok);
  let error = if ok {
    None
  } else {
    Some("one or more batched openraft membership changes failed".to_string())
  };

  Json(BatchMembershipResponse {
    ok,
    added,
    removed,
    error,
  })
}

#[derive(Deserialize)]
pub(super) struct ReplaceOpenRaftMemberRequest {
  old_node_id: NodeId,
  new_node_id: NodeId,
  new_addr: Option<String>,
  group_id: Option<String>,
  catch_up_timeout_secs: Option<u64>,
}

#[derive(Serialize)]
pub(super) struct ReplaceOpenRaftMemberResponse {
  ok: bool,
  old_node_id: NodeId,
  new_node_id: NodeId,
  groups: Vec<ReplaceOpenRaftMemberGroupResponse>,
  error: Option<String>,
}

#[derive(Serialize)]
struct ReplaceOpenRaftMemberGroupResponse {
  group_id: String,
  ok: bool,
  before_voters: Vec<NodeId>,
  after_voters: Vec<NodeId>,
  leader_id: Option<NodeId>,
  error: Option<String>,
}

/// Atomically replace one voter with another: the new node joins as a
/// learner and catches up first, then a single `change_membership` swaps
/// old for new — the group never passes through the shrunken or inflated
/// voter set that the two-step add-then-remove flow exposes.
pub(super) async fn replace_openraft_member(
  State(state): State<Arc<AppState>>,
  Json(req): Json<ReplaceOpenRaftMemberRequest>,
) -> Json<ReplaceOpenRaftMemberResponse> {
  let group_ids = resolve_membership_group_ids(&state, req.group_id.clone());
  if group_ids.is_empty() {
    return Json(ReplaceOpenRaftMemberResponse {
      ok: false,
      old_node_id: req.old_node_id,
      new_node_id: req.new_node_id,
      groups: Vec::new(),
      error: Some("openraft groups are not initialized".to_string()),
    });
  }

  if req.old_node_id == req.new_node_id {
    return Json(ReplaceOpenRaftMemberResponse {
      ok: false,
      old_node_id: req.old_node_id,
      new_node_id: req.new_node_id,
      groups: Vec::new(),
      error: Some("old_node_id and new_node_id must differ".to_string()),
    });
  }

  let new_addr =
    match resolve_openraft_member_addr(state.as_ref(), &req.new_node_id, req.new_addr).await {
      Ok(addr) => addr,
      Err(err) => {
        return Json(ReplaceOpenRaftMemberResponse {
          ok: false,
          old_node_id: req.old_node_id,
          new_node_id: req.new_node_id,
          groups: Vec::new(),
          error: Some(err),
        });
      }
    };

  if let Err(err) = state
    .network
    .register_node(req.new_node_id.clone(), &new_addr)
    .await
  {
    return Json(ReplaceOpenRaftMemberResponse {
      ok: false,
      old_node_id: req.old_node_id,
      new_node_id: req.new_node_id,
      groups: Vec::new(),
      error: Some(format!("register replacement node failed: {err}")),
    });
  }

  let catch_up_timeout = Duration::from_secs(req.catch_up_timeout_secs.unwrap_or(30));
  let mut groups = Vec::with_capacity(group_ids.len());
  for group_id in &group_ids {
    groups.push(
      replace_openraft_member_in_group(
        &state.registry,
        group_id,
        &req.old_node_id,
        &req.new_node_id,
        &new_addr,
        catch_up_timeout,
      )
      .await,
    );
  }
  let ok = groups.iter().all(|group| group.ok);
  let error = if ok {
    None
  } else {
    Some("one or more openraft membership replacements failed".to_string())
  };

  Json(ReplaceOpenRaftMemberResponse {
    ok,
    old_node_id: req.old_node_id,
    new_node_id: req.new_node_id,
    groups,
    error,
  })
}

async fn replace_openraft_member_in_group(
  registry: &crate::GroupRegistry,
  group_id: &str,
  old_node_id: &NodeId,
  new_node_id: &NodeId,
  new_addr: &str,
  catch_up_timeout: Duration,
) -> ReplaceOpenRaftMemberGroupResponse {
  let fail = |before: &BTreeSet<NodeId>, leader: Option<NodeId>, error: String| {
    ReplaceOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before.iter().cloned().collect(),
      after_voters: before.iter().cloned().collect(),
      leader_id: leader,
      error: Some(error),
    }
  };

  let Some(group) = registry.get(group_id) else {
    return fail(
      &BTreeSet::new(),
      None,
      format!("unknown group_id={group_id}"),
    );
  };

  // Fence the whole add_learner → catch-up → change_membership window: a
  // concurrent membership change interleaving inside it would race the
  // membership view captured below (TOCTOU).
  let Ok(_fence) = group.membership_fence.try_lock() else {
    return fail(
      &BTreeSet::new(),
      None,
      format!("another membership change is in progress for group {group_id}; retry later"),
    );
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();
  let before_voters = membership.voter_ids().collect::<BTreeSet<_>>();

  if !metrics.state.is_leader() {
    return fail(
      &before_voters,
      metrics.current_leader,
      "membership changes must be submitted to the leader node".to_string(),
    );
  }

  // Idempotent replay: the swap already happened.
  if !before_voters.contains(old_node_id) && before_voters.contains(new_node_id) {
    return ReplaceOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: true,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: before_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      error: None,
    };
  }

  if !before_voters.contains(old_node_id) {
    return fail(
      &before_voters,
      metrics.current_leader,
      format!("old node {old_node_id} is not a voter of group {group_id}"),
    );
  }

  let node = BasicNode {
    addr: new_addr.to_string(),
  };
  let learner_log_index = match group
    .raft
    .add_learner(new_node_id.clone(), node, false)
    .await
  {
    Ok(response) => response.log_id.index(),
    Err(err) => {
      return fail(
        &before_voters,
        metrics.current_leader,
        format!("add_learner failed: {err:?}"),
      );
    }
  };

  if let Err(err) = wait_for_openraft_member_rpc(
    registry,
    group_id,
    new_node_id,
    learner_log_index,
    catch_up_timeout,
  )
  .await
  {
    return fail(&before_voters, metrics.current_leader, err);
  }

  let after_voters = before_voters
    .iter()
    .filter(|node_id| *node_id != old_node_id)
    .cloned()
    .chain(std::iter::once(new_node_id.clone()))
    .collect::<BTreeSet<_>>();

  match group
    .raft
    .change_membership(after_voters.clone(), false)
    .await
  {
    Ok(response) => {
      tracing::info!(
        group = group_id,
        old_node_id = %old_node_id,
        new_node_id = %new_node_id,
        response = ?response,
        "replaced openraft voter in membership"
      );
      ReplaceOpenRaftMemberGroupResponse {
        group_id: group_id.to_string(),
        ok: true,
        before_voters: before_voters.iter().cloned().collect(),
        after_voters: after_voters.iter().cloned().collect(),
        leader_id: metrics.current_leader,
        error: None,
      }
    }
    Err(err) => fail(
      &before_voters,
      metrics.current_leader,
      format!("change_membership failed: {err:?}"),
    ),
  }
}

async fn resolve_openraft_member_addr(
  state: &AppState,
  node_id: &NodeId,
  requested_addr: Option<String>,
) -> Result<String, String> {
  if let Some(addr) = requested_addr {
    let (peer_id, _) =
      parse_p2p_addr(&addr).map_err(|err| format!("invalid target addr: {err}"))?;
    if node_id.as_str() != peer_id.to_string() {
      return Err(format!(
        "target node_id must match addr /p2p peer id: node_id={node_id}, peer={peer_id}"
      ));
    }
    return Ok(addr);
  }

  state
    .network
    .known_nodes()
    .await
    .into_iter()
    .find(|(known_node_id, _, _)| known_node_id == node_id)
    .map(|(_, _, addr)| addr.to_string())
    .ok_or_else(|| "target addr is required for an unknown libp2p node".to_string())
}

async fn add_openraft_member_to_group(
  registry: &crate::GroupRegistry,
  group_id: &str,
  target_node_id: &NodeId,
  target_addr: &str,
  promote: bool,
  catch_up_timeout: Duration,
  network: &Libp2pNetworkFactory,
) -> AddOpenRaftMemberGroupResponse {
  let Some(group) = registry.get(group_id) else {
    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: Vec::new(),
      after_voters: Vec::new(),
      leader_id: None,
      learner_added: false,
      promoted: false,
      error: Some(format!("unknown group_id={group_id}")),
    };
  };

  // Same fence as the replace flow: the learner-add plus optional promotion
  // below spans multiple raft calls and must not interleave with another
  // membership change on this group.
  let Ok(_fence) = group.membership_fence.try_lock() else {
    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: Vec::new(),
      after_voters: Vec::new(),
      leader_id: None,
      learner_added: false,
      promoted: false,
      error: Some(format!(
        "another membership change is in progress for group {group_id}; retry later"
      )),
    };
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();
  let before_voters = membership.voter_ids().collect::<BTreeSet<_>>();

  if !metrics.state.is_leader() {
    // Each raft group elects its own leader, so no single node is guaranteed
    // to lead every group. For learner-only additions, forward the request to
    // this group's leader instead of rejecting it.
    if !promote && let Some(leader_id) = metrics.current_leader.clone() {
      let leader_addr = membership
        .get_node(&leader_id)
        .map(|node| node.addr.clone());
      return forward_add_learner_to_leader(
        group_id,
        target_node_id,
        target_addr,
        leader_id,
        leader_addr,
        &before_voters,
        network,
      )
      .await;
    }

    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: before_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: false,
      promoted: false,
      error: Some("membership changes must be submitted to the leader node".to_string()),
    };
  }

  if before_voters.contains(target_node_id) {
    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: true,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: before_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: false,
      promoted: false,
      error: None,
    };
  }

  let node = BasicNode {
    addr: target_addr.to_string(),
  };

  let learner_log_index = match group
    .raft
    .add_learner(target_node_id.clone(), node, false)
    .await
  {
    Ok(response) => response.log_id.index(),
    Err(err) => {
      return AddOpenRaftMemberGroupResponse {
        group_id: group_id.to_string(),
        ok: false,
        before_voters: before_voters.iter().cloned().collect(),
        after_voters: before_voters.iter().cloned().collect(),
        leader_id: metrics.current_leader,
        learner_added: false,
        promoted: false,
        error: Some(format!("add_learner failed: {err:?}")),
      };
    }
  };

  if !promote {
    let metrics = group.raft.metrics().borrow_watched().clone();
    let voters = metrics
      .membership_config
      .membership()
      .voter_ids()
      .collect::<BTreeSet<_>>();
    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: true,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: true,
      promoted: false,
      error: None,
    };
  }

  if let Err(err) = wait_for_openraft_member_rpc(
    registry,
    group_id,
    target_node_id,
    learner_log_index,
    catch_up_timeout,
  )
  .await
  {
    let metrics = group.raft.metrics().borrow_watched().clone();
    let voters = metrics
      .membership_config
      .membership()
      .voter_ids()
      .collect::<BTreeSet<_>>();
    return AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: true,
      promoted: false,
      error: Some(err),
    };
  }

  let voters = group
    .raft
    .metrics()
    .borrow_watched()
    .membership_config
    .membership()
    .voter_ids()
    .chain(std::iter::once(target_node_id.clone()))
    .collect::<BTreeSet<_>>();

  match group.raft.change_membership(voters.clone(), false).await {
    Ok(response) => {
      tracing::info!(
        group = group_id,
        target_node_id = %target_node_id,
        response = ?response,
        "added openraft voter to membership"
      );
      AddOpenRaftMemberGroupResponse {
        group_id: group_id.to_string(),
        ok: true,
        before_voters: before_voters.iter().cloned().collect(),
        after_voters: voters.iter().cloned().collect(),
        leader_id: metrics.current_leader,
        learner_added: true,
        promoted: true,
        error: None,
      }
    }
    Err(err) => AddOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      learner_added: true,
      promoted: false,
      error: Some(format!("change_membership failed: {err:?}")),
    },
  }
}

/// Forward a learner-only membership addition to the given group leader over
/// libp2p. Used when the HTTP request landed on a node that is not the leader
/// of this particular raft group.
async fn forward_add_learner_to_leader(
  group_id: &str,
  target_node_id: &NodeId,
  target_addr: &str,
  leader_id: NodeId,
  leader_addr: Option<String>,
  before_voters: &BTreeSet<NodeId>,
  network: &Libp2pNetworkFactory,
) -> AddOpenRaftMemberGroupResponse {
  if let Some(addr) = leader_addr.as_ref() {
    // Best-effort: the forwarded RPC below fails with its own clear error if
    // the leader stays unreachable.
    let _ = network.register_node(leader_id.clone(), addr).await;
  }

  let response = network
    .request(
      leader_id.clone(),
      RaftRpcRequest {
        group_id: group_id.to_string(),
        op: RaftRpcOp::AddLearner(AddLearnerRequest {
          node_id: target_node_id.clone(),
          addr: target_addr.to_string(),
        }),
      },
    )
    .await;

  let (ok, learner_added, error) = match response {
    Ok(RaftRpcResponse::AddLearner(resp)) if resp.ok => (true, true, None),
    Ok(RaftRpcResponse::AddLearner(resp)) => (
      false,
      false,
      Some(
        resp
          .error
          .unwrap_or_else(|| "add_learner forwarding rejected".to_string()),
      ),
    ),
    Ok(RaftRpcResponse::Error(message)) => (
      false,
      false,
      Some(format!("forward add_learner to leader failed: {message}")),
    ),
    Ok(other) => (
      false,
      false,
      Some(format!(
        "unexpected add_learner forwarding response: {other:?}"
      )),
    ),
    Err(err) => (
      false,
      false,
      Some(format!("forward add_learner to leader failed: {err}")),
    ),
  };

  let voters: Vec<NodeId> = before_voters.iter().cloned().collect();
  AddOpenRaftMemberGroupResponse {
    group_id: group_id.to_string(),
    ok,
    before_voters: voters.clone(),
    after_voters: voters,
    leader_id: Some(leader_id),
    learner_added,
    promoted: false,
    error,
  }
}

async fn wait_for_openraft_member_rpc(
  registry: &crate::GroupRegistry,
  group_id: &str,
  target_node_id: &NodeId,
  min_matched_index: u64,
  timeout: Duration,
) -> Result<(), String> {
  let deadline = tokio::time::Instant::now() + timeout;
  loop {
    let Some(group) = registry.get(group_id) else {
      return Err(format!("unknown group_id={group_id}"));
    };

    let metrics = group.raft.metrics().borrow_watched().clone();
    if !metrics.state.is_leader() {
      return Err("local node is no longer the leader".to_string());
    }

    let leader_last_log_index = metrics.last_log_index.unwrap_or(0);
    let target_index = metrics
      .replication
      .as_ref()
      .and_then(|replication| replication.get(target_node_id))
      .and_then(|matched| matched.as_ref())
      .map(RaftLogId::index)
      .unwrap_or(0);

    let required_index = leader_last_log_index.max(min_matched_index);
    if target_index >= required_index {
      return Ok(());
    }

    if tokio::time::Instant::now() >= deadline {
      return Err(format!(
        "learner did not catch up before timeout: matched_index={target_index}, \
         required_index={required_index}"
      ));
    }

    tracing::debug!(
      group = group_id,
      target_node_id = %target_node_id,
      matched_index = target_index,
      required_index,
      "waiting for learner to catch up"
    );
    tokio::time::sleep(Duration::from_millis(500)).await;
  }
}

async fn remove_openraft_member_from_group(
  registry: &crate::GroupRegistry,
  group_id: &str,
  target_node_id: &NodeId,
) -> RemoveOpenRaftMemberGroupResponse {
  let Some(group) = registry.get(group_id) else {
    return RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: Vec::new(),
      after_voters: Vec::new(),
      leader_id: None,
      error: Some(format!("unknown group_id={group_id}")),
    };
  };

  // Fence against a concurrent multi-step membership change: the voter set
  // captured below must still be current when change_membership commits.
  let Ok(_fence) = group.membership_fence.try_lock() else {
    return RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: Vec::new(),
      after_voters: Vec::new(),
      leader_id: None,
      error: Some(format!(
        "another membership change is in progress for group {group_id}; retry later"
      )),
    };
  };

  let metrics = group.raft.metrics().borrow_watched().clone();
  let membership = metrics.membership_config.membership();
  let before_voters = membership.voter_ids().collect::<BTreeSet<_>>();
  let after_voters = before_voters
    .iter()
    .filter(|node_id| *node_id != target_node_id)
    .cloned()
    .collect::<BTreeSet<_>>();

  if !metrics.state.is_leader() {
    return RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: after_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      error: Some("membership changes must be submitted to the leader node".to_string()),
    };
  }

  if !before_voters.contains(target_node_id) {
    return RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: true,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: before_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      error: None,
    };
  }

  if after_voters.is_empty() {
    return RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: Vec::new(),
      leader_id: metrics.current_leader,
      error: Some("refusing to remove the last openraft voter".to_string()),
    };
  }

  match group
    .raft
    .change_membership(after_voters.clone(), false)
    .await
  {
    Ok(response) => {
      tracing::info!(
        group = group_id,
        target_node_id = %target_node_id,
        response = ?response,
        "removed openraft voter from membership"
      );
      RemoveOpenRaftMemberGroupResponse {
        group_id: group_id.to_string(),
        ok: true,
        before_voters: before_voters.iter().cloned().collect(),
        after_voters: after_voters.iter().cloned().collect(),
        leader_id: metrics.current_leader,
        error: None,
      }
    }
    Err(err) => RemoveOpenRaftMemberGroupResponse {
      group_id: group_id.to_string(),
      ok: false,
      before_voters: before_voters.iter().cloned().collect(),
      after_voters: after_voters.iter().cloned().collect(),
      leader_id: metrics.current_leader,
      error: Some(format!("change_membership failed: {err:?}")),
    },
  }
}
