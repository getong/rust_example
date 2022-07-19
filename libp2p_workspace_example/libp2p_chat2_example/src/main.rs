use anyhow::Result;
use futures::StreamExt;
use libp2p::{
    core::upgrade,
    floodsub::{self, Floodsub, FloodsubEvent},
    identity,
    mdns::{Mdns, MdnsEvent},
    noise,
    swarm::{NetworkBehaviourEventProcess, SwarmBuilder, SwarmEvent},
    tcp::GenTcpConfig,
    // tcp::TokioTcpConfig,
    tcp::TokioTcpTransport,
    yamux,
    Multiaddr,
    NetworkBehaviour,
    PeerId,
    Transport,
};
use tokio::io;
use tokio::io::AsyncBufReadExt;

// 自定义网络行为，组合floodsub和mDNS。
#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
struct MyBehaviour {
    floodsub: Floodsub,
    mdns: Mdns,
}

impl MyBehaviour {
    // 传入peerId，构建MyBehaviour
    async fn new(id: PeerId) -> Result<Self> {
        Ok(Self {
            // floodsub协议初始化
            floodsub: Floodsub::new(id),
            // mDNS协议初始化
            mdns: Mdns::new(Default::default()).await?,
        })
    }
}

// 处理Floodsub网络行为事件
impl NetworkBehaviourEventProcess<FloodsubEvent> for MyBehaviour {
    // 当产生一个floodsub事件时，该方法被调用。
    fn inject_event(&mut self, message: FloodsubEvent) {
        // 显示接收到的消息及来源
        if let FloodsubEvent::Message(message) = message {
            println!(
                "收到消息: '{:?}' 来自 {:?}",
                String::from_utf8_lossy(&message.data),
                message.source
            );
        }
    }
}

// 处理mDNS网络行为事件
impl NetworkBehaviourEventProcess<MdnsEvent> for MyBehaviour {
    // 当产生一个mDNS事件时，该方法被调用。
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            // 发现新节点时，将节点添加到传播消息的节点列表中。
            MdnsEvent::Discovered(list) => {
                for (peer, _) in list {
                    self.floodsub.add_node_to_partial_view(peer);
                    println!("在网络中加入节点: {peer} ");
                }
            }
            // 当节点失效时，从传播消息的节点列表中删除一个节点。
            MdnsEvent::Expired(list) => {
                for (peer, _) in list {
                    if !self.mdns.has_node(&peer) {
                        self.floodsub.remove_node_from_partial_view(&peer);
                        println!("从网络中删除节点: {peer} ");
                    }
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // 生成密钥对
    let id_keys = identity::Keypair::generate_ed25519();

    // 基于密钥对的公钥，生成节点唯一标识peerId
    let peer_id = PeerId::from(id_keys.public());
    println!("节点ID: {peer_id}");

    // 创建noise密钥对
    let noise_keys = noise::Keypair::<noise::X25519Spec>::new().into_authentic(&id_keys)?;

    // 创建一个基于tokio的TCP传输层，使用noise进行身份验证。
    // 由于多了一层加密，所以使用yamux基于TCP流进行多路复用。
    let transport = TokioTcpTransport::new(GenTcpConfig::default().nodelay(true))
        .upgrade(upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(yamux::YamuxConfig::default())
        .boxed();

    // 创建 Floodsub 主题
    let floodsub_topic = floodsub::Topic::new("chat");

    // 创建Swarm来管理节点网络及事件。
    let mut swarm = {
        let mut behaviour = MyBehaviour::new(peer_id).await?;

        // 订阅floodsub topic
        behaviour.floodsub.subscribe(floodsub_topic.clone());

        SwarmBuilder::new(transport, behaviour, peer_id)
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build()
    };

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
            line = stdin.next_line() => {
                let line = line?.expect("stdin closed");
                // 从标准输入中读取消息后，发布到订阅了floodsub topic的节点上。
                swarm.behaviour_mut().floodsub.publish(floodsub_topic.clone(), line.as_bytes());
            }
            event = swarm.select_next_some() => {
                if let SwarmEvent::NewListenAddr { address, .. } = event {
                    println!("本地监听地址: {address}");
                }
            }
        }
    }
}
