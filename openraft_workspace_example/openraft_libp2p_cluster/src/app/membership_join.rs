//! Control-cluster join protocol: probe the membership through a bootstrap
//! node, follow per-group leader redirects, and request voter admission for
//! every raft group.

use anyhow::anyhow;

use super::*;
use crate::{
  NodeId,
  network::rpc::{
    JoinClusterRequest, JoinClusterResponse, RaftRpcOp, RaftRpcRequest, RaftRpcResponse,
  },
};

pub(crate) enum JoinClusterOutcome {
  Joined,
  AlreadyMember,
  Full,
}

/// Cap on per-group leader redirects while joining, so two groups whose
/// leaders keep moving cannot bounce the join loop forever.
pub(crate) const CONTROL_JOIN_MAX_LEADER_REDIRECTS: usize = 3;

pub(crate) async fn try_join_control_cluster(
  runtime: &ControlRuntime,
  bootstrap_nodes: &[NodeId],
  advertise_addr: &str,
) -> Option<JoinClusterOutcome> {
  for bootstrap_node in bootstrap_nodes {
    if bootstrap_node == &runtime.opt.id {
      continue;
    }

    let Some(metrics) = fetch_remote_group_metrics(
      &runtime.group_ids[0],
      &runtime.opt.id,
      &runtime.libp2p.network,
    )
    .await
    else {
      tracing::debug!(
        bootstrap_node = %bootstrap_node,
        "skip control join attempt because remote metrics are unavailable"
      );
      continue;
    };

    let membership = metrics.membership_config.membership();
    if membership.get_node(&runtime.opt.id).is_some() {
      // Being in the first group is not enough: joins run group by group, so
      // an interrupted attempt can leave this node in only a prefix of the
      // groups. Only report AlreadyMember when every group contains us;
      // otherwise fall through and finish joining the remaining groups.
      if remote_openraft_voters_contain_self(
        &runtime.opt.id,
        &runtime.group_ids,
        &runtime.libp2p.network,
      )
      .await
      {
        return Some(JoinClusterOutcome::AlreadyMember);
      }
      tracing::info!(
        node_id = %runtime.opt.id,
        "node is in a subset of raft groups; resuming control join for the remaining groups"
      );
    } else {
      let voter_count = membership.voter_ids().count();
      if voter_count >= runtime.opt.max_control_nodes {
        tracing::info!(
          voters = voter_count,
          max_voters = runtime.opt.max_control_nodes,
          "openraft control membership is full; staying worker"
        );
        return Some(JoinClusterOutcome::Full);
      }
    }

    let target_node = match metrics.current_leader.as_ref() {
      Some(leader_id) if leader_id != &runtime.opt.id => leader_id.clone(),
      _ => bootstrap_node.clone(),
    };

    match request_join_all_groups(runtime, target_node, advertise_addr).await {
      Ok(outcome) => return Some(outcome),
      Err(err) => {
        tracing::debug!(error = ?err, "control join request failed");
      }
    }
  }

  None
}

pub(crate) async fn request_join_all_groups(
  runtime: &ControlRuntime,
  target_node: NodeId,
  advertise_addr: &str,
) -> anyhow::Result<JoinClusterOutcome> {
  let mut joined_any = false;

  // Each raft group elects its own leader, so a single node rarely leads all
  // groups at once. Follow leader hints per group instead of restarting the
  // whole join with a new global target (which ping-pongs forever when two
  // groups have different leaders).
  for group_id in &runtime.group_ids {
    let mut group_target = target_node.clone();
    let mut redirects = 0;

    loop {
      let response =
        request_join_control_group(runtime, group_target.clone(), group_id, advertise_addr).await?;

      if response.already_member {
        break;
      }

      if response.joined {
        joined_any = true;
        break;
      }

      if let Some(leader_id) = response.leader_id.clone()
        && leader_id != group_target
        && leader_id != runtime.opt.id
        && redirects < CONTROL_JOIN_MAX_LEADER_REDIRECTS
      {
        if let Some(leader_addr) = response.leader_addr.as_deref() {
          let _ = runtime
            .libp2p
            .network
            .register_node(leader_id.clone(), leader_addr)
            .await;
        }
        tracing::debug!(
          group = group_id,
          leader = %leader_id,
          "redirecting control join to group leader"
        );
        group_target = leader_id;
        redirects += 1;
        continue;
      }

      if response.voter_count >= response.max_voters {
        tracing::info!(
          group = group_id,
          voters = response.voter_count,
          max_voters = response.max_voters,
          error = ?response.error,
          "openraft control membership is full; staying worker"
        );
        return Ok(JoinClusterOutcome::Full);
      }

      return Err(anyhow!(
        "join group {group_id} failed: {}",
        response
          .error
          .unwrap_or_else(|| "unknown error".to_string())
      ));
    }
  }

  Ok(if joined_any {
    JoinClusterOutcome::Joined
  } else {
    JoinClusterOutcome::AlreadyMember
  })
}

pub(crate) async fn request_join_control_group(
  runtime: &ControlRuntime,
  target_node: NodeId,
  group_id: &str,
  advertise_addr: &str,
) -> anyhow::Result<JoinClusterResponse> {
  let response = runtime
    .libp2p
    .network
    .request(
      target_node.clone(),
      RaftRpcRequest {
        group_id: group_id.to_string(),
        op: RaftRpcOp::JoinCluster(JoinClusterRequest {
          node_id: runtime.opt.id.clone(),
          addr: advertise_addr.to_string(),
          max_voters: runtime.opt.max_control_nodes,
          catch_up_timeout_ms: CONTROL_JOIN_CATCH_UP_TIMEOUT_SECS * 1000,
        }),
      },
    )
    .await?;

  match response {
    RaftRpcResponse::JoinCluster(response) => Ok(response),
    RaftRpcResponse::Error(message) => Err(anyhow!("join request failed: {message}")),
    other => Err(anyhow!("unexpected join response: {other:?}")),
  }
}
