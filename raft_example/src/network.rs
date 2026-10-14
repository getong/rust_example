use std::{
  collections::{BTreeMap, HashMap},
  time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use libp2p::{
  Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
  identity::Keypair,
  noise,
  request_response::{self, ProtocolSupport},
  swarm::{
    SwarmEvent,
    dial_opts::{DialOpts, PeerCondition},
  },
  tcp, yamux,
};
use prost::Message as _;
use raft_proto::eraftpb::{Message, MessageType};

use crate::{config::NodeConfig, node::Node};

type Behaviour = request_response::cbor::Behaviour<Vec<u8>, bool>;
pub type Event = SwarmEvent<request_response::Event<Vec<u8>, bool>>;

struct Reconnect {
  at: Instant,
  delay: Duration,
  dialing: bool,
}

impl Default for Reconnect {
  fn default() -> Self {
    Self {
      at: Instant::now(),
      delay: Duration::from_millis(250),
      dialing: false,
    }
  }
}

pub struct Network {
  pub swarm: Swarm<Behaviour>,
  peers: BTreeMap<u64, (PeerId, Multiaddr)>,
  pending: HashMap<request_response::OutboundRequestId, u64>,
  reconnect: BTreeMap<u64, Reconnect>,
}

impl Network {
  pub fn new(config: &NodeConfig, key: Keypair) -> Result<Self> {
    let codec = request_response::cbor::codec::Codec::<Vec<u8>, bool>::default()
      .set_request_size_maximum(1024 * 1024)
      .set_response_size_maximum(16);
    let behaviour = Behaviour::with_codec(
      codec,
      [(
        StreamProtocol::new("/raft-example/1"),
        ProtocolSupport::Full,
      )],
      request_response::Config::default()
        .with_request_timeout(Duration::from_secs(2))
        .with_max_concurrent_streams(128),
    );
    let mut swarm = SwarmBuilder::with_existing_identity(key)
      .with_tokio()
      .with_tcp(
        tcp::Config::default().nodelay(true),
        noise::Config::new,
        yamux::Config::default,
      )?
      .with_dns()?
      .with_behaviour(|_| behaviour)?
      .with_swarm_config(|config| config.with_idle_connection_timeout(Duration::from_secs(60)))
      .build();
    swarm.listen_on(config.listen.parse()?)?;
    let mut peers = BTreeMap::new();
    for peer in &config.peers {
      if peer.id != config.id {
        peers.insert(peer.id, (peer.peer_id.parse()?, peer.address.parse()?));
      }
    }
    let reconnect = peers.keys().map(|&id| (id, Reconnect::default())).collect();
    Ok(Self {
      swarm,
      peers,
      pending: HashMap::new(),
      reconnect,
    })
  }

  // 由 Tokio tick 驱动，不依赖当前是否有 Raft 消息需要发送。
  // 每个远端只保持一个拨号尝试；连接失败使用 250ms..5s 指数退避。
  pub fn connect_peers(&mut self) {
    for (&id, (peer, address)) in &self.peers {
      let retry = self
        .reconnect
        .get_mut(&id)
        .expect("configured reconnect state");
      if self.swarm.is_connected(peer) || retry.dialing || Instant::now() < retry.at {
        continue;
      }
      let options = DialOpts::peer_id(*peer)
        // 固定地址集群不需要 TCP 打洞；避免双方复用监听端口同时拨号导致冲突。
        .allocate_new_port()
        .condition(PeerCondition::DisconnectedAndNotDialing)
        .addresses(vec![address.clone()])
        .build();
      match self.swarm.dial(options) {
        Ok(()) => retry.dialing = true,
        Err(error) => {
          retry.at = Instant::now() + retry.delay;
          eprintln!(
            "dial node {id} ({peer}) at {address} failed: {error:?}; retry in {:?}",
            retry.delay
          );
          retry.delay = (retry.delay * 2).min(Duration::from_secs(5));
        }
      }
    }
  }

  pub fn send(&mut self, messages: Vec<Message>, node: &mut Node) {
    for message in messages {
      let Some((peer, _)) = self.peers.get(&message.to) else {
        continue;
      };
      // 未连接时交给连接管理器重拨，Raft 自己重传；不为每条消息另起拨号。
      if !self.swarm.is_connected(peer) {
        node.raft.report_unreachable(message.to);
        continue;
      }
      // 有界的在途请求；慢节点不阻塞 tick 或其他节点。Raft 自己重传日志。
      if self
        .pending
        .values()
        .filter(|&&id| id == message.to)
        .count()
        >= 32
      {
        node.raft.report_unreachable(message.to);
        continue;
      }
      let request = self
        .swarm
        .behaviour_mut()
        .send_request(peer, message.encode_to_vec());
      self.pending.insert(request, message.to);
    }
  }

  pub fn handle(&mut self, event: Event, node: &mut Node) -> Result<()> {
    match event {
      SwarmEvent::NewListenAddr { address, .. } => {
        eprintln!(
          "node {} listening on {address}, peer {}",
          node.raft.raft.id,
          self.swarm.local_peer_id()
        );
      }
      SwarmEvent::ConnectionEstablished {
        peer_id, endpoint, ..
      } => {
        if let Some((&id, _)) = self.peers.iter().find(|(_, (peer, _))| *peer == peer_id) {
          self.reconnect.insert(id, Reconnect::default());
          eprintln!("connected to node {id} ({peer_id}): {endpoint:?}");
        } else {
          let _ = self.swarm.disconnect_peer_id(peer_id);
        }
      }
      SwarmEvent::OutgoingConnectionError {
        peer_id: Some(peer_id),
        error,
        ..
      } => {
        if let Some((&id, (_, address))) = self.peers.iter().find(|(_, (peer, _))| *peer == peer_id)
        {
          let retry = self
            .reconnect
            .get_mut(&id)
            .expect("configured reconnect state");
          retry.dialing = false;
          // 同时拨号时可能另一条入站连接已经成功。
          if !self.swarm.is_connected(&peer_id) {
            retry.at = Instant::now() + retry.delay;
            eprintln!(
              "dial node {id} ({peer_id}) at {address} failed: {error:?}; retry in {:?}",
              retry.delay
            );
            retry.delay = (retry.delay * 2).min(Duration::from_secs(5));
            node.raft.report_unreachable(id);
          }
        }
      }
      SwarmEvent::ConnectionClosed {
        peer_id,
        num_established: 0,
        cause,
        ..
      } => {
        if let Some((&id, _)) = self.peers.iter().find(|(_, (peer, _))| *peer == peer_id) {
          self.reconnect.insert(id, Reconnect::default());
          node.raft.report_unreachable(id);
          eprintln!("disconnected from node {id} ({peer_id}): {cause:?}; reconnecting");
        }
      }
      SwarmEvent::ListenerError { error, .. } => return Err(error.into()),
      SwarmEvent::ListenerClosed { reason, .. } => {
        reason?;
        anyhow::bail!("network listener closed");
      }
      SwarmEvent::Behaviour(request_response::Event::Message { peer, message, .. }) => {
        match message {
          request_response::Message::Request {
            request, channel, ..
          } => {
            let sender = self
              .peers
              .iter()
              .find_map(|(&id, (known, _))| (*known == peer).then_some(id));
            let result = sender
              .context("unknown peer")
              .and_then(|sender| decode(&request, sender, node.raft.raft.id))
              .and_then(|message| node.raft.step(message).map_err(Into::into));
            let accepted = result.is_ok();
            if let Err(error) = result {
              eprintln!("rejected message from {peer}: {error}");
            }
            // 只确认传输和 step 接收，不代表 Raft 已持久化或提交。
            // 真正的 Raft 响应仍由 Ready -> send 单独发送。
            let _ = self.swarm.behaviour_mut().send_response(channel, accepted);
          }
          request_response::Message::Response {
            request_id,
            response,
          } => {
            if let Some(id) = self.pending.remove(&request_id)
              && !response
            {
              node.raft.report_unreachable(id);
            }
          }
        }
      }
      SwarmEvent::Behaviour(request_response::Event::OutboundFailure {
        request_id, error, ..
      }) => {
        if let Some(id) = self.pending.remove(&request_id) {
          node.raft.report_unreachable(id);
          if !matches!(error, request_response::OutboundFailure::DialFailure) {
            eprintln!("delivery to node {id} failed: {error}");
          } // 拨号错误由 OutgoingConnectionError 输出地址和具体原因。
        }
      }
      _ => {}
    }
    Ok(())
  }
}

fn decode(bytes: &[u8], sender: u64, recipient: u64) -> Result<Message> {
  let message = Message::decode(bytes)?;
  ensure!(
    message.from == sender && message.to == recipient,
    "Raft ID does not match authenticated peer or recipient"
  );
  // 网络不能注入本地 tick/campaign/propose，且本例不支持快照/成员变更。
  ensure!(
    matches!(
      message.get_msg_type(),
      MessageType::MsgAppend
        | MessageType::MsgAppendResponse
        | MessageType::MsgRequestVote
        | MessageType::MsgRequestVoteResponse
        | MessageType::MsgRequestPreVote
        | MessageType::MsgRequestPreVoteResponse
        | MessageType::MsgHeartbeat
        | MessageType::MsgHeartbeatResponse
        | MessageType::MsgTimeoutNow
    ),
    "unsupported network message type"
  );
  Ok(message)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn protobuf_roundtrip_checks_authenticated_sender_and_recipient() {
    let mut message = Message {
      from: 1,
      to: 2,
      term: 42,
      ..Default::default()
    };
    message.set_msg_type(MessageType::MsgHeartbeat);
    let bytes = message.encode_to_vec();
    assert_eq!(decode(&bytes, 1, 2).unwrap(), message);
    assert!(decode(&bytes, 3, 2).is_err());
    assert!(decode(&bytes, 1, 3).is_err());
    assert!(decode(b"invalid protobuf", 1, 2).is_err());
    message.set_msg_type(MessageType::MsgHup);
    assert!(decode(&message.encode_to_vec(), 1, 2).is_err());
  }
}
