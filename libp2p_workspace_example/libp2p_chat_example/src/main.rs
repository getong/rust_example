use anyhow::Result;
use futures::StreamExt;
use libp2p::{
    core::transport::upgrade::Version,
    floodsub::{self, Floodsub, FloodsubEvent, Topic},
    identity,
    mdns,
    noise,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    // tcp::TokioTcpConfig,
    tcp,
    yamux,
    PeerId,
    Swarm,
    Transport,
};
use std::borrow::Cow;
use tokio::io::{stdin, AsyncBufReadExt, BufReader};

/// 处理 p2p 网络的 behavior 数据结构
/// 里面的每个域需要实现 NetworkBehaviour，或者使用 #[behaviour(ignore)]
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "ChatBehaviourEvent")]
struct ChatBehavior {
    /// flood subscription，比较浪费带宽，gossipsub 是更好的选择
    floodsub: Floodsub,
    /// 本地节点发现机制
    mdns: mdns::tokio::Behaviour,
    // 在 behavior 结构中，你也可以放其它数据，但需要 ignore
    // #[behaviour(ignore)]
    // _useless: String,
}

impl ChatBehavior {
    /// 创建一个新的 ChatBehavior
    pub async fn new(id: PeerId) -> Result<Self> {
        Ok(Self {
            mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), id).unwrap(),
            floodsub: Floodsub::new(id),
        })
    }
}

enum ChatBehaviourEvent {
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

// impl NetworkBehaviourEventProcess<FloodsubEvent> for ChatBehavior {
//     // 处理 floodsub 产生的消息
//     fn inject_event(&mut self, event: FloodsubEvent) {
//         if let FloodsubEvent::Message(msg) = event {
//             let text = String::from_utf8_lossy(&msg.data);
//             println!("{:?}: {:?}", msg.source, text);
//         }
//     }
// }

// impl NetworkBehaviourEventProcess<MdnsEvent> for ChatBehavior {
//     fn inject_event(&mut self, event: MdnsEvent) {
//         match event {
//             MdnsEvent::Discovered(list) => {
//                 // 把 mdns 发现的新的 peer 加入到 floodsub 的 view 中
//                 for (id, addr) in list {
//                     println!("Got peer: {} with addr {}", &id, &addr);
//                     self.floodsub.add_node_to_partial_view(id);
//                 }
//             }
//             MdnsEvent::Expired(list) => {
//                 // 把 mdns 发现的离开的 peer 加入到 floodsub 的 view 中
//                 for (id, addr) in list {
//                     println!("Removed peer: {} with addr {}", &id, &addr);
//                     self.floodsub.remove_node_from_partial_view(&id);
//                 }
//             }
//         }
//     }
// }

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
    let mut swarm = create_swarm(topic.clone()).await?;

    swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse()?)?;

    // 获取 stdin 的每一行
    let mut stdin = BufReader::new(stdin()).lines();

    // main loop
    loop {
        tokio::select! {
            line = stdin.next_line() => {
                let line = line?.expect("stdin closed");
                swarm.behaviour_mut().floodsub.publish(topic.clone(), line.as_bytes());
            }
            event = swarm.select_next_some() => {
                if let SwarmEvent::NewListenAddr { address, .. } = event {
                    println!("Listening on {:?}", address);
                }
            }
        }
    }
}

async fn create_swarm(topic: Topic) -> Result<Swarm<ChatBehavior>> {
    // 创建 identity（密钥对）
    let id_keys = identity::Keypair::generate_secp256k1();
    let peer_id = PeerId::from(&id_keys.public());
    println!("Local peer id: {:?}", peer_id);

    // 使用 noise protocol 来处理加密和认证
    let noise = noise::Config::new(&id_keys).unwrap();

    // 创建传输层
    let transport = tcp::tokio::Transport::default()
        .upgrade(Version::V1Lazy)
        .authenticate(noise)
        .multiplex(yamux::Config::default())
        .boxed();

    // 创建 chat behavior
    let mut behavior = ChatBehavior::new(peer_id).await?;
    // 订阅某个主题
    behavior.floodsub.subscribe(topic.clone());
    // 创建 swarm
    let swarm = SwarmBuilder::with_tokio_executor(transport, behavior, peer_id).build();

    Ok(swarm)
}
