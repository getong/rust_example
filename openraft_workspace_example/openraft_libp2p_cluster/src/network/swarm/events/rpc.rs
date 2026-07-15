//! Inbound/outbound request-response event handling for the unified RPC
//! protocol.

use std::sync::Arc;

use libp2p::{Swarm, request_response};
use tokio::sync::Semaphore;

use crate::{
  network::{
    dispatcher::SwarmRequestDispatcher,
    rpc::{UnifiedRpcRequest, UnifiedRpcResponse},
    swarm::{Behaviour, NetErr, client::CommandSenders, commands::Command, state::PendingRpcTable},
  },
  proto::raft_kv::{ErrorResponse, RaftKvResponse, raft_kv_response::Op as KvResponseOp},
};

pub(crate) fn handle_rpc_event(
  swarm: &mut Swarm<Behaviour>,
  dispatcher: Arc<dyn SwarmRequestDispatcher>,
  cmd_tx: &CommandSenders,
  dispatch_limit: Arc<Semaphore>,
  pending_rpc: &mut PendingRpcTable,
  event: request_response::Event<UnifiedRpcRequest, UnifiedRpcResponse>,
) {
  match event {
    request_response::Event::Message { message, .. } => match message {
      request_response::Message::Request {
        request, channel, ..
      } => {
        // Backpressure: when every dispatch permit is busy, non-raft
        // callers get an immediate typed "busy" signal (or a fast failure)
        // instead of queueing unboundedly in memory — they retry with
        // their own backoff. Raft requests are exempt: they are
        // rate-bounded by raft itself and rejecting a vote or heartbeat
        // would destabilize the group, so they queue on the semaphore.
        let permit = match dispatch_limit.clone().try_acquire_owned() {
          Ok(permit) => Some(permit),
          Err(_) => {
            metrics::counter!("rpc_inbound_shed_total").increment(1);
            match &request {
              UnifiedRpcRequest::Raft(_) => None,
              UnifiedRpcRequest::Kv(_) => {
                let _ = swarm.behaviour_mut().rpc.send_response(
                  channel,
                  UnifiedRpcResponse::Kv(kv_error_response("server busy; retry with backoff")),
                );
                return;
              }
              // tarpc envelopes carry no cheap error constructor here;
              // dropping the channel fails the request on the caller side
              // immediately, and both protocols already retry.
              UnifiedRpcRequest::SqliteSync(_) | UnifiedRpcRequest::Task(_) => return,
            }
          }
        };

        let dispatcher = dispatcher.clone();
        let tx = cmd_tx.clone();
        let dispatch_limit = dispatch_limit.clone();
        tokio::spawn(async move {
          // Raft fallback path: no permit was free, so wait for one inside
          // the task — the swarm loop itself never blocks. The semaphore is
          // never closed, so acquire cannot fail.
          let permit = match permit {
            Some(permit) => permit,
            None => match dispatch_limit.acquire_owned().await {
              Ok(permit) => permit,
              Err(_) => return,
            },
          };
          let resp = dispatch_unified_request(dispatcher.as_ref(), request).await;
          drop(permit);
          let _ = tx
            .for_response(&resp)
            .send(Command::RpcRespond { channel, resp })
            .await;
        });
      }
      request_response::Message::Response {
        request_id,
        response,
      } => {
        pending_rpc.complete(&request_id, Ok(response));
      }
    },
    request_response::Event::OutboundFailure {
      request_id, error, ..
    } => {
      pending_rpc.complete(
        &request_id,
        Err(NetErr(format!("outbound failure: {error}"))),
      );
    }
    request_response::Event::InboundFailure { .. } => {}
    request_response::Event::ResponseSent { .. } => {}
  }
}

/// Route an inbound unified request to the matching dispatcher handler and
/// wrap the reply in the same kind's response variant.
pub(crate) async fn dispatch_unified_request(
  dispatcher: &dyn SwarmRequestDispatcher,
  request: UnifiedRpcRequest,
) -> UnifiedRpcResponse {
  match request {
    UnifiedRpcRequest::Raft(req) => UnifiedRpcResponse::Raft(dispatcher.handle_raft(req).await),
    UnifiedRpcRequest::Kv(req) => UnifiedRpcResponse::Kv(dispatcher.handle_kv(req).await),
    UnifiedRpcRequest::SqliteSync(req) => {
      UnifiedRpcResponse::SqliteSync(dispatcher.handle_sqlite_sync(req).await)
    }
    UnifiedRpcRequest::Task(req) => UnifiedRpcResponse::Task(dispatcher.handle_task_rpc(req).await),
  }
}

pub(crate) fn kv_error_response(message: impl Into<String>) -> RaftKvResponse {
  RaftKvResponse {
    op: Some(KvResponseOp::Error(ErrorResponse {
      message: message.into(),
      leader_id: String::new(),
    })),
  }
}
