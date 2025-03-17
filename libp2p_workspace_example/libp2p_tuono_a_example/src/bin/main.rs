// # Step 1: Generate the private key in PEM format
// openssl ecparam -name secp256k1 -genkey -noout -out private_key.pem

// # Step 2: Convert the PEM key to raw hex (ensure it's 32 bytes)
// openssl ec -in private_key.pem -noout -text |
// grep 'priv:' -A 3 |
// sed '1d' |
// tr -d '\n[:space:]:' |
// head -c 64 > identity.txt
//
// # Optionally, remove the PEM file
// rm private_key.pem

use std::{collections::HashSet, error::Error, fs};

use anyhow::Result;
use futures::StreamExt;
use libp2p::{
  core::ConnectedPoint,
  identify::{
    Behaviour as IdentifyBehavior, Config as IdentifyConfig, Event as IdentifyEvent,
    Info as IdentifyInfo,
  },
  identity::{self, Keypair},
  kad::{
    self, store::MemoryStore as KadInMemory, Behaviour as KadBehavior, Config as KadConfig,
    Event as KadEvent,
  },
  noise,
  ping::{Behaviour as PingBehaviour, Config as PingConfig, Event as PingEvent},
  request_response::{
    self, Event as RequestResponseEvent, OutboundRequestId, ProtocolSupport, ResponseChannel,
  },
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux, PeerId, StreamProtocol, Swarm, SwarmBuilder,
};
use libp2p_tuono_a_example::{
  libp2p::{
    behaviour::{RaftRequest, RaftResponse},
    LAZY_EVENT_SENDER, RECEIVER_GROUP,
  },
  openraft::{start_example_raft_node, LAZY_RAFT},
};
use tokio::{
  sync::{
    mpsc::{channel, Receiver as MpscReceiver, Sender as MpscSender},
    oneshot::{Receiver as OneshotReceiver, Sender as OneshotSender},
  },
  time::Duration,
};

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "MyBehaviourEvent")]
pub struct MyBehaviour {
  pub kad: kad::Behaviour<kad::store::MemoryStore>,
  pub identify: IdentifyBehavior,
  pub ping: PingBehaviour,
  pub request_response: request_response::json::Behaviour<RaftRequest, RaftResponse>,
}

// TODO modify two peer id here
const PEER_ID_LIST: [&str; 2] = [
  "16Uiu2HAmEQsneQVk7AtniZfUXdNTHgcv4RPSj62K6Dm3mjguRfDS",
  "16Uiu2HAmKW8CcvFg7uZGb4DUmUdugywUHJPF3dqmDBquPjNXTrKP",
];

impl MyBehaviour {
  fn new(peer_id: PeerId, key: Keypair) -> Result<Self> {
    let mut kad_config = KadConfig::new(StreamProtocol::new("/agent/connection/1.0.0"));
    kad_config.set_periodic_bootstrap_interval(Some(Duration::from_secs(120)));
    kad_config.set_publication_interval(Some(Duration::from_secs(120)));
    kad_config.set_replication_interval(Some(Duration::from_secs(120)));
    let kad_memory = KadInMemory::new(peer_id);
    let kad = KadBehavior::with_config(peer_id, kad_memory, kad_config);

    let identify_config =
      IdentifyConfig::new("/agent/connection/1.0.0".to_string(), key.clone().public())
        .with_push_listen_addr_updates(true)
        .with_interval(Duration::from_secs(120));
    let identify = IdentifyBehavior::new(identify_config);
    let ping = PingBehaviour::new(
      PingConfig::new()
        .with_interval(Duration::from_secs(10))
        .with_timeout(Duration::from_secs(10)),
    );
    let request_response = request_response::json::Behaviour::<RaftRequest, RaftResponse>::new(
      [(
        StreamProtocol::new("/my-json-protocol"),
        ProtocolSupport::Full,
      )],
      request_response::Config::default(),
    );

    Ok(Self {
      kad,
      identify,
      ping,
      request_response,
    })
  }

  pub fn known_peers(&mut self) -> HashSet<PeerId> {
    let mut peers = HashSet::new();
    for b in self.kad.kbuckets() {
      for e in b.iter() {
        if !peers.contains(e.node.key.preimage()) {
          peers.insert(*e.node.key.preimage());
        }
      }
    }

    peers
  }
}

#[derive(Debug)]
pub enum MyBehaviourEvent {
  Kad(KadEvent),
  Identify(IdentifyEvent),
  Ping(PingEvent),
  RequestResponse(RequestResponseEvent<RaftRequest, RaftResponse>),
}

impl From<KadEvent> for MyBehaviourEvent {
  fn from(event: KadEvent) -> Self {
    MyBehaviourEvent::Kad(event)
  }
}

impl From<IdentifyEvent> for MyBehaviourEvent {
  fn from(value: IdentifyEvent) -> Self {
    Self::Identify(value)
  }
}

impl From<PingEvent> for MyBehaviourEvent {
  fn from(value: PingEvent) -> Self {
    Self::Ping(value)
  }
}

impl From<RequestResponseEvent<RaftRequest, RaftResponse>> for MyBehaviourEvent {
  fn from(value: RequestResponseEvent<RaftRequest, RaftResponse>) -> Self {
    Self::RequestResponse(value)
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  _ = start_example_raft_node(1, "data").await;
  let key_pair = make_libp2p_keypair().await.unwrap();

  let mut swarm = SwarmBuilder::with_existing_identity(key_pair)
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_behaviour(|key| {
      let peer_id = PeerId::from(key.public());
      println!("peer id : {}", peer_id.to_base58());
      MyBehaviour::new(peer_id, key.clone()).unwrap()
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(15)))
    .build();

  swarm.listen_on("/ip4/0.0.0.0/tcp/9090".parse()?)?;
  swarm.behaviour_mut().kad.set_mode(Some(kad::Mode::Server));
  let mut interval1 = tokio::time::interval(Duration::from_secs(3));
  let mut interval2 = tokio::time::interval(Duration::from_secs(2));
  let (event_sender, mut event_receiver) =
    channel::<(RaftRequest, OneshotSender<RaftResponse>)>(10);
  {
    let mut event_lock = LAZY_EVENT_SENDER.lock().await;
    *event_lock = Some(event_sender);
  }
  loop {
    tokio::select! {
          Some((request, oneshot_sender)) = event_receiver.recv() => {
              let req_id = swarm.behaviour_mut().request_response.send_request(&PeerId::random(), request);
              let mut req_group = RECEIVER_GROUP.lock().await;
              req_group.insert(req_id, oneshot_sender);
              drop(req_group);
          }
        event = swarm.select_next_some() => {
            handle_event(&mut swarm, event).await;
        }

        _ = interval1.tick() => {
            let peer_list = swarm.behaviour_mut().known_peers();
            println!("\n------ 3 secs, local_peer_id is {:?}, kad peer list is {:?}\n\n", swarm.local_peer_id(), peer_list);
        }

        _ = interval2.tick() => {
            let mut peer_list = HashSet::new();
            for peer in swarm.connected_peers() {
                peer_list.insert(peer);
            }
            println!("\n------ 2 secs, local_peer_id is {:?}, swarm peer list is {:?}\n\n", swarm.local_peer_id(), peer_list);
        }
    }
  }
}

pub async fn handle_event(swarm: &mut Swarm<MyBehaviour>, event: SwarmEvent<MyBehaviourEvent>) {
  println!("\n------event is {:?}-----\n", event);
  match event {
        SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(event)) => {
            match event {
                RequestResponseEvent::Message { message, ..} => {
                    match message {
                        request_response::Message::Request { request, channel,.. } => {
                            if let Ok(resp) = handle_open_raft_request(request).await {
                                swarm
                                    .behaviour_mut()
                                    .request_response
                                    .send_response(channel, resp)
                                    .expect("Connection to peer to be still open.");
                            }
                        }
                        request_response::Message::Response {
                            request_id,
                            response,
                        } => {
                            let mut req_group = RECEIVER_GROUP.lock().await;
                            if let Some(sender) = req_group.remove(&request_id) {
                                let _ = sender.send(response);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        SwarmEvent::NewListenAddr {
            // listener_id,
            // address,
            ..
        } => {

        }
        SwarmEvent::NewExternalAddrOfPeer { peer_id, address } => {
            println!("peer_id : {}, address: {}", peer_id, address);
        }
        SwarmEvent::ConnectionEstablished { .. } => {}
        SwarmEvent::ConnectionClosed {
            peer_id,
            num_established,
            endpoint,
            ..
        } => {
            if num_established == 0 {
                swarm.behaviour_mut().kad.remove_peer(&peer_id);
                match endpoint {
                    ConnectedPoint::Listener { send_back_addr, .. } => {
                        println!("send_back_addr: {}", send_back_addr);
                    }
                    _ => {}
                }
            }
        }
        SwarmEvent::Behaviour(MyBehaviourEvent::Identify(sub_event)) => {
            handle_identify_event(swarm, sub_event).await
        }

        _ => {}
    }
}

async fn handle_identify_event(swarm: &mut Swarm<MyBehaviour>, event: IdentifyEvent) {
  println!("event: {:?}, file: {}, line: {}", event, file!(), line!());
  if let IdentifyEvent::Received {
    peer_id,
    info: IdentifyInfo { listen_addrs, .. },
    ..
  } = event
  {
    let peer_str = peer_id.to_base58();
    if PEER_ID_LIST.contains(&peer_str.as_str()) {
      for addr in listen_addrs {
        swarm.behaviour_mut().kad.add_address(&peer_id, addr);
      }
    }
  }
}

async fn make_libp2p_keypair() -> Result<Keypair, Box<dyn Error>> {
  let file_path = "identity.txt";
  let private_key_str = fs::read_to_string(file_path)?.trim().to_string();

  if private_key_str.is_empty() {
    return Err(format!("Private key is empty in file: {}", file_path).into());
  }
  let private_key_str = if private_key_str.starts_with("0x") {
    &private_key_str[2 ..]
  } else {
    &private_key_str
  };

  // Decode the hex string into bytes
  let private_key_bytes = hex::decode(private_key_str)?;

  // Check if the length of the private key is valid
  if private_key_bytes.len() != 32 {
    return Err("Private key must be exactly 32 bytes".into());
  }

  // Create a libp2p Keypair from the secp256k1 private key
  let secret_key = identity::secp256k1::SecretKey::try_from_bytes(private_key_bytes)?;
  let libp2p_keypair = identity::secp256k1::Keypair::from(secret_key).into();
  Ok(libp2p_keypair)
}

async fn handle_open_raft_request(
  request: RaftRequest,
) -> Result<RaftResponse, Box<dyn std::error::Error>> {
  let raft_lock = LAZY_RAFT.lock();
  match raft_lock.await.as_ref() {
    Some(raft) => match request {
      RaftRequest::AppendEntries(req) => {
        let resp = raft
          .append_entries(req)
          .await
          .map_err(Box::<dyn std::error::Error>::from)?;
        Ok(RaftResponse::AppendEntries(resp))
      }
      RaftRequest::InstallSnapshot(req) => {
        let resp = raft
          .install_snapshot(req)
          .await
          .map_err(Box::<dyn std::error::Error>::from)?;
        Ok(RaftResponse::InstallSnapshot(resp))
      }
      RaftRequest::Vote(req) => {
        let resp = raft
          .vote(req)
          .await
          .map_err(Box::<dyn std::error::Error>::from)?;
        Ok(RaftResponse::Vote(resp))
      }
    },
    _ => Err("not found".into()),
  }
}
