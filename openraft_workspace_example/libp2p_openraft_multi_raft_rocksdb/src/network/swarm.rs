use std::{
  collections::{HashMap, HashSet},
  error::Error,
  fmt,
  time::Duration,
};

use futures::StreamExt;
use libp2p::{
  Multiaddr, PeerId, Swarm, gossipsub,
  kad::{self, store::MemoryStore},
  mdns, ping,
  request_response::{self, OutboundRequestId, ResponseChannel},
  swarm::{NetworkBehaviour, SwarmEvent},
};
use openraft::async_runtime::WatchReceiver;
use openraft_rocksstore_crud::RocksRequest;
use prost::Message;
use tokio::sync::{mpsc, oneshot};

use crate::{
  GroupHandleMap, Unreachable,
  network::{
    proto_codec::{ProstCodec, ProtoCodec},
    rpc::{RaftRpcOp, RaftRpcRequest, RaftRpcResponse},
    transport::{Libp2pNetworkFactory, parse_p2p_addr},
  },
  proto::raft_kv::{
    ChatMessage, ErrorResponse, RaftKvRequest, RaftKvResponse, raft_kv_request::Op as KvRequestOp,
    raft_kv_response::Op as KvResponseOp,
  },
  signal::ShutdownRx,
  store::{KvData, ensure_linearizable_read},
  typ::{Raft, Snapshot},
};

pub const GOSSIP_TOPIC: &str = "openraft/cluster/1";

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
  pub gossipsub: gossipsub::Behaviour,
  pub ping: ping::Behaviour,
  pub mdns: mdns::tokio::Behaviour,
  pub kad: kad::Behaviour<MemoryStore>,
}

#[derive(Debug)]
pub enum BehaviourEvent {
  Raft(request_response::Event<RaftRpcRequest, RaftRpcResponse>),
  Kv(request_response::Event<RaftKvRequest, RaftKvResponse>),
  Gossipsub(gossipsub::Event),
  Ping(ping::Event),
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

impl From<gossipsub::Event> for BehaviourEvent {
  fn from(value: gossipsub::Event) -> Self {
    Self::Gossipsub(value)
  }
}

impl From<ping::Event> for BehaviourEvent {
  fn from(value: ping::Event) -> Self {
    Self::Ping(value)
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
  GossipsubPublish {
    topic: String,
    data: Vec<u8>,
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

  pub async fn publish_gossipsub(&self, topic: &str, data: Vec<u8>) -> Result<(), NetErr> {
    self
      .tx
      .send(Command::GossipsubPublish {
        topic: topic.to_string(),
        data,
      })
      .await
      .map_err(|e| NetErr(format!("command channel closed: {e}")))
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
  network: Libp2pNetworkFactory,
  groups: GroupHandleMap,
  kv_client: KvClient,
  mut shutdown_rx: ShutdownRx,
) {
  let mut pending_raft: HashMap<
    OutboundRequestId,
    oneshot::Sender<Result<RaftRpcResponse, NetErr>>,
  > = HashMap::new();
  let mut pending_kv: HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>> =
    HashMap::new();
  let mut connected_peers: HashSet<PeerId> = HashSet::new();
  let mut reconnect_tick = tokio::time::interval(Duration::from_secs(12));
  let mut kad_discovery_tick = tokio::time::interval(Duration::from_secs(30));
  reconnect_tick.tick().await;
  kad_discovery_tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm");
        break;
      }
      _ = reconnect_tick.tick() => {
        handle_reconnect_tick(&mut swarm, &network, &connected_peers).await;
      }
      _ = kad_discovery_tick.tick() => {
        kick_kad_queries(&mut swarm);
      }
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { return; };
        handle_command(&mut swarm, cmd, &mut pending_raft, &mut pending_kv);
      }

      ev = swarm.select_next_some() => {
        handle_swarm_event(
          &mut swarm,
          ev,
          &network,
          &groups,
          &kv_client,
          &cmd_tx,
          &mut pending_raft,
          &mut pending_kv,
          &mut connected_peers,
        )
        .await;
      }
    }
  }
}

async fn handle_reconnect_tick(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  connected_peers: &HashSet<PeerId>,
) {
  let nodes = network.known_nodes().await;
  for (_node_id, peer_id, addr) in nodes {
    if peer_id == *swarm.local_peer_id() {
      continue;
    }
    if connected_peers.contains(&peer_id) {
      continue;
    }
    tracing::info!(
      peer = %peer_id,
      addr = %addr,
      "reconnecting to peer"
    );
    let _ = Swarm::dial(swarm, addr.clone());
    add_kad_address_from_p2p(swarm, &addr);
  }
}

fn handle_command(
  swarm: &mut Swarm<Behaviour>,
  cmd: Command,
  pending_raft: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftRpcResponse, NetErr>>>,
  pending_kv: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>>,
) {
  match cmd {
    Command::Dial { addr } => {
      let dial_addr = addr.clone();
      let _ = Swarm::dial(swarm, dial_addr);
      add_kad_address_from_p2p(swarm, &addr);
      kick_kad_queries(swarm);
    }
    Command::GossipsubPublish { topic, data } => {
      let topic = gossipsub::IdentTopic::new(topic);
      if let Err(err) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
        tracing::warn!("gossipsub publish failed: {err}");
      }
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

async fn handle_swarm_event(
  swarm: &mut Swarm<Behaviour>,
  event: SwarmEvent<BehaviourEvent>,
  network: &Libp2pNetworkFactory,
  groups: &GroupHandleMap,
  kv_client: &KvClient,
  cmd_tx: &mpsc::Sender<Command>,
  pending_raft: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftRpcResponse, NetErr>>>,
  pending_kv: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>>,
  connected_peers: &mut HashSet<PeerId>,
) {
  match event {
    SwarmEvent::Behaviour(BehaviourEvent::Raft(event)) => {
      handle_raft_event(swarm, groups, cmd_tx, pending_raft, event).await;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Kv(event)) => {
      handle_kv_event(swarm, groups, kv_client, cmd_tx, pending_kv, event);
    }
    SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => {
      handle_mdns_event(swarm, network, event).await;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(event)) => {
      handle_gossipsub_event(event);
    }
    SwarmEvent::Behaviour(BehaviourEvent::Ping(event)) => {
      handle_ping_event(event);
    }
    SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
      handle_kad_event(swarm, Some(connected_peers), event);
    }
    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
      handle_connection_established(swarm, connected_peers, peer_id);
    }
    SwarmEvent::ConnectionClosed {
      peer_id,
      num_established,
      cause,
      ..
    } => {
      handle_connection_closed(swarm, connected_peers, peer_id, num_established, cause);
    }
    SwarmEvent::NewListenAddr { address, .. } => {
      tracing::info!("listening on {address}");
    }
    _ => {}
  }
}

async fn handle_raft_event(
  swarm: &mut Swarm<Behaviour>,
  groups: &GroupHandleMap,
  cmd_tx: &mpsc::Sender<Command>,
  pending_raft: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftRpcResponse, NetErr>>>,
  event: request_response::Event<RaftRpcRequest, RaftRpcResponse>,
) {
  match event {
    request_response::Event::Message { message, .. } => match message {
      request_response::Message::Request {
        request, channel, ..
      } => {
        let group_id = request.group_id.clone();
        let Some(group) = groups.get(&group_id) else {
          let _ = swarm.behaviour_mut().raft.send_response(
            channel,
            RaftRpcResponse::Error(format!("unknown group_id={group_id}")),
          );
          return;
        };

        match request.op {
          RaftRpcOp::ClientWrite(req) => {
            let raft = group.raft.clone();
            let tx = cmd_tx.clone();
            tokio::spawn(async move {
              let resp = RaftRpcResponse::ClientWrite(raft.client_write(req).await);
              let _ = tx.send(Command::RaftRespond { channel, resp }).await;
            });
          }
          other => {
            let resp = handle_inbound_rpc(group.raft.clone(), other).await;
            let _ = swarm.behaviour_mut().raft.send_response(channel, resp);
          }
        }
      }
      request_response::Message::Response {
        request_id,
        response,
      } => {
        if let Some(tx) = pending_raft.remove(&request_id) {
          let _ = tx.send(Ok(response));
        }
      }
    },
    request_response::Event::OutboundFailure {
      request_id, error, ..
    } => {
      if let Some(tx) = pending_raft.remove(&request_id) {
        let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
      }
    }
    request_response::Event::InboundFailure { .. } => {}
    request_response::Event::ResponseSent { .. } => {}
  }
}

fn handle_kv_event(
  _swarm: &mut Swarm<Behaviour>,
  groups: &GroupHandleMap,
  kv_client: &KvClient,
  cmd_tx: &mpsc::Sender<Command>,
  pending_kv: &mut HashMap<OutboundRequestId, oneshot::Sender<Result<RaftKvResponse, NetErr>>>,
  event: request_response::Event<RaftKvRequest, RaftKvResponse>,
) {
  match event {
    request_response::Event::Message { message, .. } => match message {
      request_response::Message::Request {
        request, channel, ..
      } => {
        let group_id = request.group_id.clone();
        if group_id.is_empty() {
          let resp = kv_error_response("missing group_id");
          let tx = cmd_tx.clone();
          tokio::spawn(async move {
            let _ = tx.send(Command::KvRespond { channel, resp }).await;
          });
          return;
        }

        let Some(group) = groups.get(&group_id) else {
          let resp = kv_error_response(format!("unknown group_id={group_id}"));
          let tx = cmd_tx.clone();
          tokio::spawn(async move {
            let _ = tx.send(Command::KvRespond { channel, resp }).await;
          });
          return;
        };

        let raft = group.raft.clone();
        let kv_data = group.kv_data.clone();
        let kv_client = kv_client.clone();
        let tx = cmd_tx.clone();
        tokio::spawn(async move {
          let resp = handle_inbound_kv(raft, kv_data, kv_client, request).await;
          let _ = tx.send(Command::KvRespond { channel, resp }).await;
        });
      }
      request_response::Message::Response {
        request_id,
        response,
      } => {
        if let Some(tx) = pending_kv.remove(&request_id) {
          let _ = tx.send(Ok(response));
        }
      }
    },
    request_response::Event::OutboundFailure {
      request_id, error, ..
    } => {
      if let Some(tx) = pending_kv.remove(&request_id) {
        let _ = tx.send(Err(NetErr(format!("outbound failure: {error}"))));
      }
    }
    request_response::Event::InboundFailure { .. } => {}
    request_response::Event::ResponseSent { .. } => {}
  }
}

async fn handle_mdns_event(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  event: mdns::Event,
) {
  match event {
    mdns::Event::Discovered(list) => {
      let mut saw_peer = false;
      for (peer, addr) in list {
        saw_peer = true;
        network.update_peer_addr_from_mdns(peer, addr.clone()).await;
        add_kad_peer_address(swarm, peer, addr);
        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
      }
      if saw_peer {
        kick_kad_queries(swarm);
      }
    }
    mdns::Event::Expired(list) => {
      for (peer, addr) in list {
        let addr = strip_p2p(addr);
        swarm.behaviour_mut().kad.remove_address(&peer, &addr);
        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer);
      }
    }
  }
}

fn handle_gossipsub_event(event: gossipsub::Event) {
  match event {
    gossipsub::Event::Message {
      propagation_source,
      message_id,
      message,
    } => match ChatMessage::decode(message.data.as_slice()) {
      Ok(chat) => {
        tracing::info!(
          peer = %propagation_source,
          message_id = %message_id,
          from = %chat.from,
          text = %chat.text,
          ts = chat.ts_unix_ms,
          "chat message"
        );
      }
      Err(err) => {
        tracing::info!(
          peer = %propagation_source,
          message_id = %message_id,
          len = message.data.len(),
          error = %err,
          "gossipsub message (decode failed)"
        );
      }
    },
    other => {
      tracing::debug!("gossipsub event: {:?}", other);
    }
  }
}

fn handle_ping_event(event: ping::Event) {
  match event {
    ping::Event {
      peer,
      result: Ok(rtt),
      ..
    } => {
      tracing::debug!(peer = %peer, rtt = ?rtt, "ping ok");
    }
    ping::Event {
      peer,
      result: Err(err),
      ..
    } => {
      tracing::warn!(peer = %peer, error = ?err, "ping failed");
    }
  }
}

fn handle_connection_established(
  swarm: &mut Swarm<Behaviour>,
  connected_peers: &mut HashSet<PeerId>,
  peer_id: PeerId,
) {
  connected_peers.insert(peer_id);
  swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
  kick_kad_queries(swarm);
}

fn handle_connection_closed<E: fmt::Display>(
  swarm: &mut Swarm<Behaviour>,
  connected_peers: &mut HashSet<PeerId>,
  peer_id: PeerId,
  num_established: u32,
  cause: Option<E>,
) {
  if num_established == 0 {
    connected_peers.remove(&peer_id);
    swarm
      .behaviour_mut()
      .gossipsub
      .remove_explicit_peer(&peer_id);
    if let Some(cause) = cause {
      tracing::warn!(peer = %peer_id, error = %cause, "connection closed");
    } else {
      tracing::info!(peer = %peer_id, "connection closed");
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
  let mut kad_discovery_tick = tokio::time::interval(Duration::from_secs(30));
  kad_discovery_tick.tick().await;

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm client");
        break;
      }
      _ = kad_discovery_tick.tick() => {
        kick_kad_queries(&mut swarm);
      }
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else { return; };
        match cmd {
          Command::Dial { addr } => {
            let dial_addr = addr.clone();
            let _ = Swarm::dial(&mut swarm, dial_addr);
            add_kad_address_from_p2p(&mut swarm, &addr);
            kick_kad_queries(&mut swarm);
          }
          Command::GossipsubPublish { topic, data } => {
            let topic = gossipsub::IdentTopic::new(topic);
            if let Err(err) = swarm.behaviour_mut().gossipsub.publish(topic, data) {
              tracing::warn!("gossipsub publish failed: {err}");
            }
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
              let mut saw_peer = false;
              for (peer, addr) in list {
                saw_peer = true;
                add_kad_peer_address(&mut swarm, peer, addr);
                swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
              }
              if saw_peer {
                kick_kad_queries(&mut swarm);
              }
            }
            mdns::Event::Expired(list) => {
              for (peer, addr) in list {
                let addr = strip_p2p(addr);
                swarm.behaviour_mut().kad.remove_address(&peer, &addr);
                swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer);
              }
            }
          },

          SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(event)) => match event {
            gossipsub::Event::Message {
              propagation_source,
              message_id,
              message,
            } => {
              match ChatMessage::decode(message.data.as_slice()) {
                Ok(chat) => {
                  tracing::info!(
                    peer = %propagation_source,
                    message_id = %message_id,
                    from = %chat.from,
                    text = %chat.text,
                    ts = chat.ts_unix_ms,
                    "chat message"
                  );
                }
                Err(err) => {
                  tracing::info!(
                    peer = %propagation_source,
                    message_id = %message_id,
                    len = message.data.len(),
                    error = %err,
                    "gossipsub message (decode failed)"
                  );
                }
              }
            }
            other => {
              tracing::debug!("gossipsub event: {:?}", other);
            }
          },

          SwarmEvent::Behaviour(BehaviourEvent::Ping(event)) => {
            match event {
              ping::Event { peer, result: Ok(rtt), .. } => {
                tracing::debug!(peer = %peer, rtt = ?rtt, "ping ok");
              }
              ping::Event { peer, result: Err(err), .. } => {
                tracing::warn!(peer = %peer, error = ?err, "ping failed");
              }
            }
          }

          SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
            handle_kad_event(&mut swarm, None, event);
          }

          SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
            kick_kad_queries(&mut swarm);
          }

          SwarmEvent::ConnectionClosed { peer_id, num_established, .. } => {
            if num_established == 0 {
              swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
            }
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

fn ensure_p2p_addr(mut addr: Multiaddr, peer: PeerId) -> Multiaddr {
  if matches!(
    addr.iter().last(),
    Some(libp2p::multiaddr::Protocol::P2p(_))
  ) {
    return addr;
  }
  addr.push(libp2p::multiaddr::Protocol::P2p(peer.into()));
  addr
}

fn kick_kad_queries(swarm: &mut Swarm<Behaviour>) {
  let local_peer_id = swarm.local_peer_id().to_owned();
  let _ = swarm.behaviour_mut().kad.bootstrap();
  swarm.behaviour_mut().kad.get_closest_peers(local_peer_id);
}

fn handle_kad_event(
  swarm: &mut Swarm<Behaviour>,
  connected_peers: Option<&HashSet<PeerId>>,
  event: kad::Event,
) {
  match event {
    kad::Event::RoutingUpdated {
      peer, addresses, ..
    } => {
      if peer == *swarm.local_peer_id() {
        return;
      }
      let Some(connected_peers) = connected_peers else {
        tracing::debug!(peer = %peer, "kad routing updated (client)");
        return;
      };
      if connected_peers.contains(&peer) {
        return;
      }
      for addr in addresses.iter() {
        let dial_addr = ensure_p2p_addr(addr.clone(), peer);
        let _ = Swarm::dial(swarm, dial_addr);
      }
    }
    kad::Event::OutboundQueryProgressed { result, .. } => {
      if let kad::QueryResult::GetClosestPeers(result) = result {
        match result {
          Ok(ok) => {
            if ok.peers.is_empty() {
              tracing::debug!("kad get_closest_peers complete: no peers");
            } else {
              tracing::debug!(peers = ?ok.peers, "kad get_closest_peers complete");
            }
          }
          Err(err) => {
            tracing::debug!(error = ?err, "kad get_closest_peers failed");
          }
        }
      }
    }
    other => {
      tracing::debug!("kad event: {:?}", other);
    }
  }
}

async fn handle_inbound_kv(
  raft: Raft,
  kv_data: KvData,
  kv_client: KvClient,
  request: RaftKvRequest,
) -> RaftKvResponse {
  if request.group_id.is_empty() {
    return kv_error_response("missing group_id");
  }

  let metrics = raft.metrics().borrow_watched().clone();
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

async fn handle_inbound_rpc(raft: Raft, request: RaftRpcOp) -> RaftRpcResponse {
  match request {
    RaftRpcOp::AppendEntries(req) => {
      let res = raft.append_entries(req).await;
      RaftRpcResponse::AppendEntries(res)
    }
    RaftRpcOp::Vote(req) => {
      let res = raft.vote(req).await;
      RaftRpcResponse::Vote(res)
    }
    RaftRpcOp::ClientWrite(req) => {
      let res = raft.client_write(req).await;
      RaftRpcResponse::ClientWrite(res)
    }
    RaftRpcOp::GetMetrics => {
      let metrics = raft.metrics().borrow_watched().clone();
      RaftRpcResponse::GetMetrics(metrics)
    }
    RaftRpcOp::FullSnapshot { vote, meta, data } => {
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
