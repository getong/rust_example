use anyhow::Result;
use futures::StreamExt;

use libp2p::{
  floodsub::{self, Floodsub, FloodsubEvent},
  identity, mdns, noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux, Multiaddr, PeerId, SwarmBuilder,
};
use tokio::io;
use tokio::io::AsyncBufReadExt;
use tokio::time::Duration;

// 自定义网络行为，组合floodsub和mDNS。
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "MyBehaviourEvent")]
struct MyBehaviour {
  floodsub: Floodsub,
  mdns: mdns::tokio::Behaviour,
}

impl MyBehaviour {
  // 传入peerId，构建MyBehaviour
  fn new(id: PeerId) -> Result<Self> {
    Ok(Self {
      // floodsub协议初始化
      floodsub: Floodsub::new(id),
      // mDNS协议初始化
      mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), id).unwrap(),
    })
  }
}

pub enum MyBehaviourEvent {
  FloodSub(FloodsubEvent),
  Mdns(mdns::Event),
}

impl From<FloodsubEvent> for MyBehaviourEvent {
  fn from(event: FloodsubEvent) -> Self {
    MyBehaviourEvent::FloodSub(event)
  }
}

impl From<mdns::Event> for MyBehaviourEvent {
  fn from(event: mdns::Event) -> Self {
    MyBehaviourEvent::Mdns(event)
  }
}

#[tokio::main]
async fn main() -> Result<()> {
  // 生成密钥对
  let id_keys = identity::Keypair::generate_secp256k1();

  // 创建 Floodsub 主题
  let floodsub_topic = floodsub::Topic::new("chat");

  let mut swarm = SwarmBuilder::with_existing_identity(id_keys.clone())
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_dns()?
    .with_behaviour(|key| {
      let peer_id = PeerId::from(key.public());
      MyBehaviour::new(peer_id).unwrap()
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(5)))
    .build();

  swarm
    .behaviour_mut()
    .floodsub
    .subscribe(floodsub_topic.clone());

  // 指定一个远程节点，进行手动链接。
  if let Some(to_dial) = std::env::args().nth(1) {
    let addr: Multiaddr = to_dial.parse()?;
    swarm.dial(addr)?;
    println!("链接远程节点: {to_dial}");
  }

  // 从标准输入中读取消息
  let mut stdin = io::BufReader::new(io::stdin()).lines();

  // 监听操作系统分配的端口
  swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse()?)?;

  loop {
    tokio::select! {
      Ok(Some(line)) = stdin.next_line() => {
        swarm.behaviour_mut().floodsub.publish(floodsub_topic.clone(), line);
      }
      event = swarm.select_next_some() => {
            if let SwarmEvent::NewListenAddr { address, .. } = event {
                println!("本地监听地址: {address}");
            }
        }
    }
  }
}
