use std::{collections::HashMap, error::Error, fmt, time::Duration};

use futures::StreamExt;
use libp2p::{
  Multiaddr, PeerId, Swarm,
  kad::{self, store::MemoryStore},
  mdns,
  request_response::{self, OutboundRequestId, ResponseChannel},
  swarm::{NetworkBehaviour, SwarmEvent},
};
use openraft::error::Unreachable;
use openraft_rocksstore_crud::RocksRequest;
use tokio::sync::{mpsc, oneshot};

use crate::{
  network::{
    proto_codec::{ProstCodec, ProtoCodec},
    rpc::{RaftRpcRequest, RaftRpcResponse},
    transport::parse_p2p_addr,
  },
  proto::raft_kv::{
    ErrorResponse, RaftKvRequest, RaftKvResponse, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  signal::ShutdownRx,
  store::{KvData, ensure_linearizable_read},
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
  pub raft: request_response::Behaviour<ProtoCodec>,
  pub kv: request_response::Behaviour<ProstCodec<RaftKvRequest, RaftKvResponse>>,
  pub mdns: mdns::tokio::Behaviour,
  pub kad: kad::Behaviour<MemoryStore>,
}

#[derive(Debug)]
pub enum BehaviourEvent {
  Raft(request_response::Event<RaftRpcRequest, RaftRpcResponse>),
  Kv(request_response::Event<RaftKvRequest, RaftKvResponse>),
  Mdns(mdns::Event),
  Kad(kad::Event),
}

impl From<request_response::Event<RaftRpcRequest, RaftRpcResponse>> for BehaviourEvent {
  fn from(value: request_response::Event<RaftRpcRequest, RaftRpcResponse>) -> Self {
    Self::Raft(value)
  }
}

impl From<request_response::Event<RaftKvRequest, RaftKvResponse>> for BehaviourEvent {
  fn from(value: request_response::Event<RaftKvRequest, RaftKvResponse>) -> Self {
    Self::Kv(value)
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
  RaftRequest {
    peer: PeerId,
    req: RaftRpcRequest,
    resp: oneshot::Sender<Result<RaftRpcResponse, NetErr>>,
  },
  RaftRespond {
    channel: ResponseChannel<RaftRpcResponse>,
    resp: RaftRpcResponse,
  },
  KvRequest {
    peer: PeerId,
    req: RaftKvRequest,
    resp: oneshot::Sender<Result<RaftKvResponse, NetErr>>,
  },
  KvRespond {
    channel: ResponseChannel<RaftKvResponse>,
    resp: RaftKvResponse,
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
      .send(Command::RaftRequest {
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

#[derive(Clone)]
pub struct KvClient {
  tx: mpsc::Sender<Command>,
  timeout: Duration,
}

impl KvClient {
  pub fn new(tx: mpsc::Sender<Command>, timeout: Duration) -> Self {
    Self { tx, timeout }
  }

  pub async fn dial(&self, addr: Multiaddr) {
    let _ = self.tx.send(Command::Dial { addr }).await;
  }

  pub async fn request(
    &self,
    peer: PeerId,
    req: RaftKvRequest,
  ) -> Result<RaftKvResponse, Unreachable> {
    let (resp_tx, resp_rx) = oneshot::channel();
    self
      .tx
      .send(Command::KvRequest {
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
  cmd_tx: mpsc::Sender<Command>,
  raft: Raft,
  kv_data: KvData,
  kv_client: KvClient,
  mut shutdown_rx: ShutdownRx,
) {
  let mut pending_raft: HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<RaftRpcResponse, NetErr>>,
  > = HashMap::new();
  let mut pending_kv: HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>> =
    HashMap::new();

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm");
        break;
      }
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { return; };
        match cmd {
          Command::Dial { addr } => {
            let dial_addr = addr.clone();
            let _ = Swarm::dial(&mut swarm, dial_addr);
            add_kad_address_from_p2p(&mut swarm, &addr);
          }
          Command::RaftRequest { peer, req, resp } => {
            let id = swarm.behaviour_mut().raft.send_request(&peer, req);
            pending_raft.insert(id, resp);
          }
          Command::RaftRespond { channel, resp } => {
            let _ = swarm.behaviour_mut().raft.send_response(channel, resp);
          }
          Command::KvRequest { peer, req, resp } => {
            let id = swarm.behaviour_mut().kv.send_request(&peer, req);
            pending_kv.insert(id, resp);
          }
          Command::KvRespond { channel, resp } => {
            let _ = swarm.behaviour_mut().kv.send_response(channel, resp);
          }
        }
      }

      ev = swarm.select_next_some() => {
        match ev {
          SwarmEvent::Behaviour(BehaviourEvent::Raft(event)) => match event {
            request_response::Event::Message { message, .. } => match message {
              request_response::Message::Request { request, channel, .. } => {
                match request {
                  RaftRpcRequest::ClientWrite(req) => {
                    let raft = raft.clone();
                    let tx = cmd_tx.clone();
                    tokio::spawn(async move {
                      let resp = RaftRpcResponse::ClientWrite(raft.client_write(req).await);
                      let _ = tx.send(Command::RaftRespond { channel, resp }).await;
                    });
                  }
                  other => {
                    let resp = handle_inbound_rpc(raft.clone(), other).await;
                    let _ = swarm.behaviour_mut().raft.send_response(channel, resp);
                  }
                }
              }
              request_response::Message::Response { request_id, response } => {
                if let Some(tx) = pending_raft.remove(&request_id) {
                  let _ = tx.send(Ok(response));
                }
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              if let Some(tx) = pending_raft.remove(&request_id) {
                let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
              }
            }

            request_response::Event::InboundFailure { .. } => {}
            request_response::Event::ResponseSent { .. } => {}
          },

          SwarmEvent::Behaviour(BehaviourEvent::Kv(event)) => match event {
            request_response::Event::Message { message, .. } => match message {
              request_response::Message::Request { request, channel, .. } => {
                let raft = raft.clone();
                let kv_data = kv_data.clone();
                let kv_client = kv_client.clone();
                let tx = cmd_tx.clone();
                tokio::spawn(async move {
                  let resp = handle_inbound_kv(raft, kv_data, kv_client, request).await;
                  let _ = tx.send(Command::KvRespond { channel, resp }).await;
                });
              }
              request_response::Message::Response { request_id, response } => {
                if let Some(tx) = pending_kv.remove(&request_id) {
                  let _ = tx.send(Ok(response));
                }
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              if let Some(tx) = pending_kv.remove(&request_id) {
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
pub async fn run_swarm_client(swarm: Swarm<Behaviour>, cmd_rx: mpsc::Receiver<Command>) {
  let (_shutdown_tx, shutdown_rx) = crate::signal::channel();
  run_swarm_client_with_shutdown(swarm, cmd_rx, shutdown_rx).await;
}

pub async fn run_swarm_client_with_shutdown(
  mut swarm: Swarm<Behaviour>,
  mut cmd_rx: mpsc::Receiver<Command>,
  mut shutdown_rx: ShutdownRx,
) {
  let mut pending_raft: HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<RaftRpcResponse, NetErr>>,
  > = HashMap::new();
  let mut pending_kv: HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>> =
    HashMap::new();

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm client");
        break;
      }
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { return; };
        match cmd {
          Command::Dial { addr } => {
            let dial_addr = addr.clone();
            let _ = Swarm::dial(&mut swarm, dial_addr);
            add_kad_address_from_p2p(&mut swarm, &addr);
          }
          Command::RaftRequest { peer, req, resp } => {
            let id = swarm.behaviour_mut().raft.send_request(&peer, req);
            pending_raft.insert(id, resp);
          }
          Command::RaftRespond { channel, resp } => {
            let _ = swarm.behaviour_mut().raft.send_response(channel, resp);
          }
          Command::KvRequest { peer, req, resp } => {
            let id = swarm.behaviour_mut().kv.send_request(&peer, req);
            pending_kv.insert(id, resp);
          }
          Command::KvRespond { channel, resp } => {
            let _ = swarm.behaviour_mut().kv.send_response(channel, resp);
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
                if let Some(tx) = pending_raft.remove(&request_id) {
                  let _ = tx.send(Ok(response));
                }
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              if let Some(tx) = pending_raft.remove(&request_id) {
                let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
              }
            }

            request_response::Event::InboundFailure { .. } => {}
            request_response::Event::ResponseSent { .. } => {}
          },

          SwarmEvent::Behaviour(BehaviourEvent::Kv(event)) => match event {
            request_response::Event::Message { message, .. } => match message {
              request_response::Message::Request { channel, .. } => {
                let _ = swarm
                  .behaviour_mut()
                  .kv
                  .send_response(channel, kv_error_response("client-only"));
              }
              request_response::Message::Response { request_id, response } => {
                if let Some(tx) = pending_kv.remove(&request_id) {
                  let _ = tx.send(Ok(response));
                }
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              if let Some(tx) = pending_kv.remove(&request_id) {
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

async fn handle_inbound_kv(
  raft: Raft,
  kv_data: KvData,
  kv_client: KvClient,
  request: RaftKvRequest,
) -> RaftKvResponse {
  let metrics = raft.metrics().borrow().clone();
  if !metrics.state.is_leader() {
    let Some(leader_id) = metrics.current_leader else {
      return kv_error_response("no leader available");
    };
    let Some(node) = metrics.membership_config.membership().get_node(&leader_id) else {
      return kv_error_response("leader node not found in membership");
    };
    let Ok((peer, addr)) = parse_p2p_addr(&node.addr) else {
      return kv_error_response("invalid leader address");
    };
    kv_client.dial(addr).await;
    return match kv_client.request(peer, request).await {
      Ok(resp) => resp,
      Err(err) => kv_error_response(format!("forward to leader failed: {err}")),
    };
  }

  let Some(op) = request.op else {
    return kv_error_response("missing request op");
  };

  match op {
    KvRequestOp::Get(req) => {
      if let Err(err) = ensure_linearizable_read(&raft).await {
        return kv_error_response(format!("{err:?}"));
      }
      let kvs = kv_data.read().await;
      match kvs.get(&req.key) {
        Some(value) => RaftKvResponse {
          op: Some(KvResponseOp::Get(crate::proto::raft_kv::GetValueResponse {
            found: true,
            value: value.clone(),
          })),
        },
        None => RaftKvResponse {
          op: Some(KvResponseOp::Get(crate::proto::raft_kv::GetValueResponse {
            found: false,
            value: String::new(),
          })),
        },
      }
    }
    KvRequestOp::Set(req) => {
      let key = req.key;
      let value = req.value;
      match raft
        .client_write(RocksRequest::Set {
          key,
          value: value.clone(),
        })
        .await
      {
        Ok(resp) => RaftKvResponse {
          op: Some(KvResponseOp::Set(crate::proto::raft_kv::SetValueResponse {
            ok: true,
            value: resp.data.value.unwrap_or(value),
          })),
        },
        Err(err) => kv_error_response(format!("{err:?}")),
      }
    }
    KvRequestOp::Update(req) => {
      let key = req.key;
      let value = req.value;
      if let Err(err) = ensure_linearizable_read(&raft).await {
        return kv_error_response(format!("{err:?}"));
      }
      let exists = {
        let kvs = kv_data.read().await;
        kvs.contains_key(&key)
      };
      if !exists {
        RaftKvResponse {
          op: Some(KvResponseOp::Update(
            crate::proto::raft_kv::UpdateValueResponse {
              ok: false,
              value: String::new(),
            },
          )),
        }
      } else {
        match raft
          .client_write(RocksRequest::Update {
            key,
            value: value.clone(),
          })
          .await
        {
          Ok(resp) => RaftKvResponse {
            op: Some(KvResponseOp::Update(
              crate::proto::raft_kv::UpdateValueResponse {
                ok: true,
                value: resp.data.value.unwrap_or(value),
              },
            )),
          },
          Err(err) => kv_error_response(format!("{err:?}")),
        }
      }
    }
    KvRequestOp::Delete(req) => {
      if let Err(err) = ensure_linearizable_read(&raft).await {
        return kv_error_response(format!("{err:?}"));
      }
      let exists = {
        let kvs = kv_data.read().await;
        kvs.contains_key(&req.key)
      };
      if !exists {
        RaftKvResponse {
          op: Some(KvResponseOp::Delete(
            crate::proto::raft_kv::DeleteValueResponse { ok: false },
          )),
        }
      } else {
        match raft
          .client_write(RocksRequest::Delete { key: req.key })
          .await
        {
          Ok(_) => RaftKvResponse {
            op: Some(KvResponseOp::Delete(
              crate::proto::raft_kv::DeleteValueResponse { ok: true },
            )),
          },
          Err(err) => kv_error_response(format!("{err:?}")),
        }
      }
    }
  }
}

fn kv_error_response(message: impl Into<String>) -> RaftKvResponse {
  RaftKvResponse {
    op: Some(KvResponseOp::Error(ErrorResponse {
      message: message.into(),
    })),
  }
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
    RaftRpcRequest::ClientWrite(req) => {
      let res = raft.client_write(req).await;
      RaftRpcResponse::ClientWrite(res)
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
          openraft::error::RaftError::<
            openraft_rocksstore_crud::TypeConfig,
            openraft::error::Infallible,
          >::Fatal(e)
        });

      RaftRpcResponse::FullSnapshot(res)
    }
  }
}
