use std::collections::HashSet;

use anyhow::Result;
use futures::StreamExt;
use libp2p::{
  identity,
  kad::{self, store::MemoryStore, Event as KademliaEvent, Quorum, Record, RecordKey},
  mdns, noise,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux, PeerId, SwarmBuilder,
};
// use libp2p_kad::Event as KademliaEvent;
use tokio::io::{self, AsyncBufReadExt};
use tokio::time::Duration;

// 自定义网络行为，组合Kademlia和mDNS.
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "MyBehaviourEvent")]
struct MyBehaviour {
  // kademlia: Kademlia<MemoryStore>,
  kademlia: kad::Behaviour<kad::store::MemoryStore>,
  mdns: mdns::tokio::Behaviour,
}

impl MyBehaviour {
  // 传入peerId，构建MyBehaviour
  fn new(peer_id: PeerId) -> Result<Self> {
    let kademlia = kad::Behaviour::new(peer_id, MemoryStore::new(peer_id));

    Ok(Self {
      // floodsub协议初始化
      kademlia,
      // mDNS协议初始化
      mdns: mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id).unwrap(),
    })
  }

  pub fn known_peers(&mut self) -> HashSet<PeerId> {
    let mut peers = HashSet::new();
    for b in self.kademlia.kbuckets() {
      for e in b.iter() {
        if !peers.contains(e.node.key.preimage()) {
          peers.insert(*e.node.key.preimage());
        }
      }
    }

    peers
  }
}

pub enum MyBehaviourEvent {
  Kademlia(KademliaEvent),
  Mdns(mdns::Event),
}

impl From<KademliaEvent> for MyBehaviourEvent {
  fn from(event: KademliaEvent) -> Self {
    MyBehaviourEvent::Kademlia(event)
  }
}

impl From<mdns::Event> for MyBehaviourEvent {
  fn from(event: mdns::Event) -> Self {
    MyBehaviourEvent::Mdns(event)
  }
}

// // 处理mDNS网络行为事件
// impl NetworkBehaviourEventProcess<MdnsEvent> for MyBehaviour {
//     // 当产生一个mDNS事件时，该方法被调用。
//     fn inject_event(&mut self, event: MdnsEvent) {
//         // 发现新节点时，将节点加入到Kademlia网络中。
//         if let MdnsEvent::Discovered(list) = event {
//             for (peer_id, multiaddr) in list {
//                 self.kademlia.add_address(&peer_id, multiaddr);
//             }
//         }
//     }
// }

// // 处理Kademlia网络行为事件
// impl NetworkBehaviourEventProcess<KademliaEvent> for MyBehaviour {
//     // 当产生一个kademlia事件时，该方法被调用。
//     fn inject_event(&mut self, message: KademliaEvent) {
//         if let KademliaEvent::OutboundQueryCompleted { result, .. } = message {
//             match result {
//                 // 查询提供key的节点事件
//                 QueryResult::GetProviders(Ok(ok)) => {
//                     for peer in ok.providers {
//                         println!(
//                             "节点 {:?} 提供了key {:?}",
//                             peer,
//                             std::str::from_utf8(ok.key.as_ref()).unwrap()
//                         );
//                     }
//                 }
//                 QueryResult::GetProviders(Err(err)) => {
//                     eprintln!("Failed to get providers: {:?}", err);
//                 }
//                 // 查询存储记录事件
//                 QueryResult::GetRecord(Ok(ok)) => {
//                     for PeerRecord {
//                         record: Record { key, value, .. },
//                         ..
//                     } in ok.records
//                     {
//                         println!(
//                             "获取存储记录 {:?} {:?}",
//                             std::str::from_utf8(key.as_ref()).unwrap(),
//                             std::str::from_utf8(&value).unwrap(),
//                         );
//                     }
//                 }
//                 QueryResult::GetRecord(Err(err)) => {
//                     eprintln!("Failed to get record: {:?}", err);
//                 }
//                 // 记录存储成功事件
//                 QueryResult::PutRecord(Ok(PutRecordOk { key })) => {
//                     println!(
//                         "成功存储记录  {:?}",
//                         std::str::from_utf8(key.as_ref()).unwrap()
//                     );
//                 }
//                 QueryResult::PutRecord(Err(err)) => {
//                     eprintln!("Failed to put record: {:?}", err);
//                 }
//                 // 成功存储记录提供者事件
//                 QueryResult::StartProviding(Ok(AddProviderOk { key })) => {
//                     println!(
//                         "成功存储记录提供者 {:?}",
//                         std::str::from_utf8(key.as_ref()).unwrap()
//                     );
//                 }
//                 QueryResult::StartProviding(Err(err)) => {
//                     eprintln!("Failed to put provider record: {:?}", err);
//                 }
//                 _ => {}
//             }
//         }
//     }
// }

#[tokio::main]
async fn main() -> Result<()> {
  // 生成密钥对
  let key_pair = identity::Keypair::generate_ed25519();

  // 基于密钥对的公钥，生成节点唯一标识peerId
  // let peer_id = PeerId::from(key_pair.public());
  // println!("节点ID: {peer_id}");

  // 在Mplex协议上建立一个加密的，启用dns的TCP传输
  // let transport = libp2p::development_transport(key_pair).await?;

  // 创建Swarm网络管理器，来管理节点网络及事件。
  // let mut swarm = {
  //   let behaviour = MyBehaviour::new(peer_id).await?;

  //   SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build()
  // };
  let mut swarm = SwarmBuilder::with_existing_identity(key_pair)
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_behaviour(|key| {
      let peer_id = PeerId::from(key.public());
      MyBehaviour::new(peer_id).unwrap()
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
    .build();

  // 从标准输入中读取消息
  let mut stdin = io::BufReader::new(io::stdin()).lines();

  // 监听操作系统分配的端口
  swarm.listen_on("/ip4/127.0.0.1/tcp/0".parse()?)?;

  loop {
    let mut internal_timer1 = tokio::time::interval(Duration::from_secs(3));
    let mut internal_timer2 = tokio::time::interval(Duration::from_secs(5));
    tokio::select! {
    line = stdin.next_line() => {
        let line = line?.expect("stdin closed");
        handle_input_line(&mut swarm.behaviour_mut().kademlia, line);
    },
    event = swarm.select_next_some() => {
        if let SwarmEvent::NewListenAddr { address, .. } = event {
            println!("本地监听地址: {address}");
        }
    }
    _ = internal_timer1.tick() => {
        let mut peers = HashSet::new();
        for connected_peer in swarm.connected_peers() {
            peers.insert(connected_peer.clone());
        }

        for i in peers.iter() {
            println!("swarm connected peer is {:?}", i);
        }
    }
    _ = internal_timer2.tick() => {
        let peers = swarm.behaviour_mut().known_peers();
        for i in peers.iter() {
            println!("swarm kad connected peer is {:?}", i);
        }
    }}
  }
}

// 处理输入命令
fn handle_input_line(kademlia: &mut kad::Behaviour<kad::store::MemoryStore>, line: String) {
  let mut args = line.split(' ');

  match args.next() {
    // 处理 GET 命令，获取存储的kv记录
    Some("GET") => {
      let key = {
        match args.next() {
          Some(key) => RecordKey::new(&key),
          None => {
            eprintln!("Expected key");
            return;
          }
        }
      };
      // 获取v记录
      kademlia.get_record(key);
    }
    // 处理 GET_PROVIDERS 命令，获取存储kv记录的节点PeerId
    Some("GET_PROVIDERS") => {
      let key = {
        match args.next() {
          Some(key) => RecordKey::new(&key),
          None => {
            eprintln!("Expected key");
            return;
          }
        }
      };
      // 获取存储kv记录的节点
      kademlia.get_providers(key);
    }
    // 处理 PUT 命令，存储kv记录
    Some("PUT") => {
      let key = {
        match args.next() {
          Some(key) => RecordKey::new(&key),
          None => {
            eprintln!("Expected key");
            return;
          }
        }
      };
      // 将值转换成Vec<u8>类型
      let value = {
        match args.next() {
          Some(value) => value.as_bytes().to_vec(),
          None => {
            eprintln!("Expected value");
            return;
          }
        }
      };
      let record = Record {
        key,
        value,
        publisher: None,
        expires: None,
      };
      // 存储kv记录
      kademlia
        .put_record(record, Quorum::One)
        .expect("Failed to store record locally.");
    }
    // 处理 PUT_PROVIDER 命令，保存kv记录的提供者(节点)
    Some("PUT_PROVIDER") => {
      let key = {
        match args.next() {
          Some(key) => RecordKey::new(&key),
          None => {
            eprintln!("Expected key");
            return;
          }
        }
      };

      kademlia
        .start_providing(key)
        .expect("Failed to start providing key");
    }
    _ => {
      eprintln!("expected GET, GET_PROVIDERS, PUT or PUT_PROVIDER");
    }
  }
}
