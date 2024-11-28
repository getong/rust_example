use std::borrow::Cow;

use anyhow::Result;
use libp2p::{
  floodsub::{self, Floodsub, FloodsubEvent},
  futures::StreamExt,
  identity, mdns, noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux, PeerId, Swarm, SwarmBuilder,
};
use tokio::{
  io::{stdin, AsyncBufReadExt, BufReader},
  time::Duration,
};

/// 处理 p2p 网络的 behavior 数据结构
/// 里面的每个域需要实现 NetworkBehaviour，或者使用 #[behaviour(ignore)]
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "ChatBehaviourEvent")]
struct ChatBehavior {
  /// flood subscription，比较浪费带宽，gossipsub 是更好的选择
  floodsub: Floodsub,
  /// 本地节点发现机制
  mdns: mdns::tokio::Behaviour,
}

impl ChatBehavior {
  /// 创建一个新的 ChatBehavior
  pub fn new(id: PeerId) -> Result<Self> {
    Ok(Self {
      mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), id).unwrap(),
      floodsub: Floodsub::new(id),
    })
  }
}

pub enum ChatBehaviourEvent {
  FloodSub(FloodsubEvent),
  Mdns(mdns::Event),
}

impl From<FloodsubEvent> for ChatBehaviourEvent {
  fn from(event: FloodsubEvent) -> Self {
    ChatBehaviourEvent::FloodSub(event)
  }
}

impl From<mdns::Event> for ChatBehaviourEvent {
  fn from(event: mdns::Event) -> Self {
    ChatBehaviourEvent::Mdns(event)
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  // 如果带参数，当成一个 topic
  let name = match std::env::args().nth(1) {
    Some(arg) => Cow::Owned(arg),
    None => Cow::Borrowed("lobby"),
  };

  // 创建 floodsub topic
  let topic = floodsub::Topic::new(name);

  // 创建 swarm
  let mut swarm = create_swarm().await?;
  swarm.behaviour_mut().floodsub.subscribe(topic.clone());

  swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse()?)?;

  // 获取 stdin 的每一行
  let mut stdin = BufReader::new(stdin()).lines();

  // main loop
  loop {
    tokio::select! {
      Ok(Some(line)) = stdin.next_line() => {
        swarm.behaviour_mut().floodsub.publish(topic.clone(), line);
        }
      event = swarm.select_next_some() => {
            if let SwarmEvent::NewListenAddr { address, .. } = event {
                println!("Listening on {:?}", address);
            }
        }
    }
  }
}

async fn create_swarm() -> Result<Swarm<ChatBehavior>> {
  // 创建 identity（密钥对）
  let id_keys = identity::Keypair::generate_secp256k1();
  let swarm = SwarmBuilder::with_existing_identity(id_keys.clone())
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_dns()?
    .with_behaviour(|key| {
      let peer_id = PeerId::from(key.public());
      ChatBehavior::new(peer_id).unwrap()
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
    .build();

  Ok(swarm)
}
