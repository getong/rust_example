use std::{collections::HashMap, error::Error, fmt, time::Duration};

use futures::StreamExt;
use libp2p::{
  Multiaddr, PeerId, Swarm,
  kad::{self, store::MemoryStore},
  mdns,
  request_response::{self, OutboundRequestId},
  swarm::{NetworkBehaviour, SwarmEvent},
};
use openraft::error::Unreachable;
use tokio::sync::{mpsc, oneshot};

use crate::{
  network::rpc::{RaftRpcRequest, RaftRpcResponse},
  typ::{Raft, Snapshot},
};

#[derive(Debug)]
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
  pub raft: request_response::cbor::Behaviour<RaftRpcRequest, RaftRpcResponse>,
  pub mdns: mdns::tokio::Behaviour,
  pub kad: kad::Behaviour<MemoryStore>,
}

#[derive(Debug)]
pub enum BehaviourEvent {
  Raft(request_response::Event<RaftRpcRequest, RaftRpcResponse>),
  Mdns(mdns::Event),
  Kad(kad::Event),
}

impl From<request_response::Event<RaftRpcRequest, RaftRpcResponse>> for BehaviourEvent {
  fn from(value: request_response::Event<RaftRpcRequest, RaftRpcResponse>) -> Self {
    Self::Raft(value)
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
  Request {
    peer: PeerId,
    req: RaftRpcRequest,
    resp: oneshot::Sender<Result<RaftRpcResponse, NetErr>>,
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

  pub async fn request(
    &self,
    peer: PeerId,
    req: RaftRpcRequest,
  ) -> Result<RaftRpcResponse, Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::Request {
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
  mut swarm: Swarm<Behaviour>,
  mut cmd_rx: mpsc::Receiver<Command>,
  raft: Raft,
) {
  let mut pending: HashMap<OutboundRequestId, oneshot::Sender<Result<RaftRpcResponse, NetErr>>> =
    HashMap::new();

  loop {
    tokio::select! {
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { return; };
        match cmd {
          Command::Dial { addr } => {
            let dial_addr = addr.clone();
            let _ = Swarm::dial(&mut swarm, dial_addr);
            add_kad_address_from_p2p(&mut swarm, &addr);
          }
          Command::Request { peer, req, resp } => {
            let id = swarm.behaviour_mut().raft.send_request(&peer, req);
            pending.insert(id, resp);
          }
        }
      }

      ev = swarm.select_next_some() => {
        match ev {
          SwarmEvent::Behaviour(BehaviourEvent::Raft(event)) => match event {
            request_response::Event::Message { message, .. } => match message {
              request_response::Message::Request { request, channel, .. } => {
                let resp = handle_inbound_rpc(raft.clone(), request).await;
                let _ = swarm.behaviour_mut().raft.send_response(channel, resp);
              }
              request_response::Message::Response { request_id, response } => {
                if let Some(tx) = pending.remove(&request_id) {
                  let _ = tx.send(Ok(response));
                }
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              if let Some(tx) = pending.remove(&request_id) {
                let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
              }
            }

            request_response::Event::InboundFailure { .. } => {}
            request_response::Event::ResponseSent { .. } => {}
          },

          SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => match event {
            mdns::Event::Discovered(list) => {
              for (peer, addr) in list {
                add_kad_peer_address(&mut swarm, peer, addr);
              }
            }
            mdns::Event::Expired(list) => {
              for (peer, addr) in list {
                let addr = strip_p2p(addr);
                swarm.behaviour_mut().kad.remove_address(&peer, &addr);
              }
            }
          },

          SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
            tracing::debug!("kad event: {:?}", event);
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

/// Client-only swarm loop.
///
/// It supports outbound requests/responses but does not require a `Raft` handle.
/// If it receives an inbound request, it responds with `RaftRpcResponse::Error`.
pub async fn run_swarm_client(mut swarm: Swarm<Behaviour>, mut cmd_rx: mpsc::Receiver<Command>) {
  let mut pending: HashMap<OutboundRequestId, oneshot::Sender<Result<RaftRpcResponse, NetErr>>> =
    HashMap::new();

  loop {
    tokio::select! {
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { return; };
        match cmd {
          Command::Dial { addr } => {
            let dial_addr = addr.clone();
            let _ = Swarm::dial(&mut swarm, dial_addr);
            add_kad_address_from_p2p(&mut swarm, &addr);
          }
          Command::Request { peer, req, resp } => {
            let id = swarm.behaviour_mut().raft.send_request(&peer, req);
            pending.insert(id, resp);
          }
        }
      }

      ev = swarm.select_next_some() => {
        match ev {
          SwarmEvent::Behaviour(BehaviourEvent::Raft(event)) => match event {
            request_response::Event::Message { message, .. } => match message {
              request_response::Message::Request { channel, .. } => {
                let _ = swarm
                  .behaviour_mut()
                  .raft
                  .send_response(channel, RaftRpcResponse::Error("client-only".to_string()));
              }
              request_response::Message::Response { request_id, response } => {
                if let Some(tx) = pending.remove(&request_id) {
                  let _ = tx.send(Ok(response));
                }
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              if let Some(tx) = pending.remove(&request_id) {
                let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
              }
            }

            request_response::Event::InboundFailure { .. } => {}
            request_response::Event::ResponseSent { .. } => {}
          },

          SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => match event {
            mdns::Event::Discovered(list) => {
              for (peer, addr) in list {
                add_kad_peer_address(&mut swarm, peer, addr);
              }
            }
            mdns::Event::Expired(list) => {
              for (peer, addr) in list {
                let addr = strip_p2p(addr);
                swarm.behaviour_mut().kad.remove_address(&peer, &addr);
              }
            }
          },

          SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
            tracing::debug!("kad event: {:?}", event);
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

async fn handle_inbound_rpc(raft: Raft, request: RaftRpcRequest) -> RaftRpcResponse {
  match request {
    RaftRpcRequest::AppendEntries(req) => {
      let res = raft.append_entries(req).await;
      RaftRpcResponse::AppendEntries(res)
    }
    RaftRpcRequest::Vote(req) => {
      let res = raft.vote(req).await;
      RaftRpcResponse::Vote(res)
    }
    RaftRpcRequest::GetMetrics => {
      let metrics = raft.metrics().borrow().clone();
      RaftRpcResponse::GetMetrics(metrics)
    }
    RaftRpcRequest::FullSnapshot { vote, meta, data } => {
      let snapshot = Snapshot {
        meta,
        snapshot: std::io::Cursor::new(data),
      };

      // Match the error-mapping pattern used elsewhere in this workspace.
      let res = raft
        .install_full_snapshot(vote, snapshot)
        .await
        .map_err(|e| {
          openraft::error::RaftError::<openraft_rocksstore::TypeConfig, openraft::error::Infallible>::Fatal(e)
        });

      RaftRpcResponse::FullSnapshot(res)
    }
  }
}
