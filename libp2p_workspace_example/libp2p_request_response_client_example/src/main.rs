use futures::StreamExt;
use libp2p::{
  identity, kad, noise,
  request_response::{self, ProtocolSupport},
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux, Multiaddr, PeerId, StreamProtocol, Swarm,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "MyBehaviourEvent")]
struct Behaviour {
  request_response: request_response::cbor::Behaviour<FileRequest, FileResponse>,
  kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

#[derive(Debug)]
pub enum MyBehaviourEvent {
  RequestResponse(request_response::Event<FileRequest, FileResponse>),
  Kademlia(kad::Event),
}

impl From<request_response::Event<FileRequest, FileResponse>> for MyBehaviourEvent {
  fn from(event: request_response::Event<FileRequest, FileResponse>) -> Self {
    MyBehaviourEvent::RequestResponse(event)
  }
}

impl From<kad::Event> for MyBehaviourEvent {
  fn from(event: kad::Event) -> Self {
    MyBehaviourEvent::Kademlia(event)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRequest(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileResponse(Vec<u8>);

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Generate a random keypair for the local peer
  let local_key = identity::Keypair::generate_ed25519();
  let local_peer_id = PeerId::from(local_key.public());
  println!("Local peer id: {:?}", local_peer_id);

  // Define the server's PeerId and Multiaddr here. Replace these with actual values.
  let server_peer_id: PeerId = "12D3KooWG2xD9LHpKg5hsvYcJQoJBEMQ4ptFWtoZCDQN7orDoUzC"
    .parse()
    .unwrap();
  let server_address: Multiaddr = "/ip4/127.0.0.1/tcp/12345".parse().unwrap();

  let mut swarm = libp2p::SwarmBuilder::with_existing_identity(local_key)
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )?
    .with_behaviour(|key| Behaviour {
      kademlia: kad::Behaviour::new(
        local_peer_id,
        kad::store::MemoryStore::new(key.public().to_peer_id()),
      ),
      request_response: request_response::cbor::Behaviour::new(
        [(
          StreamProtocol::new("/file-exchange/1"),
          ProtocolSupport::Full,
        )],
        request_response::Config::default(),
      ),
    })?
    .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
    .build();

  // Add server's address to the Kademlia DHT and connect
  swarm
    .behaviour_mut()
    .kademlia
    .add_address(&server_peer_id, server_address.clone());

  // Dial the server
  if let Err(e) = Swarm::dial(&mut swarm, server_address) {
    eprintln!("Dialing the server failed: {:?}", e);
    return Ok(());
  }

  println!("Dialing the server...");

  loop {
    match swarm.next().await.unwrap() {
      // Handle RequestResponse events
      SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(
        request_response::Event::Message {
          peer,
          message: request_response::Message::Response { response, .. },
        },
      )) => {
        println!("Received response from {:?}: {:?}", peer, response.0);
        break; // Exit the loop after receiving the response
      }
      SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(
        request_response::Event::OutboundFailure { peer, error, .. },
      )) => {
        eprintln!("Failed to send request to {:?}: {:?}", peer, error);
        break;
      }
      SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(
        request_response::Event::InboundFailure { peer, error, .. },
      )) => {
        eprintln!("Inbound failure from {:?}: {:?}", peer, error);
      }
      SwarmEvent::Behaviour(MyBehaviourEvent::RequestResponse(
        request_response::Event::ResponseSent { peer, .. },
      )) => {
        println!("Response sent to {:?}", peer);
      }
      SwarmEvent::ConnectionEstablished { peer_id, .. } => {
        println!("Connected to {:?}", peer_id);
        if peer_id == server_peer_id {
          // Send a file request
          let request = FileRequest("ping".to_string());
          swarm
            .behaviour_mut()
            .request_response
            .send_request(&peer_id, request);
          println!("Sent request to {:?}", peer_id);
        }
      }
      SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
        println!("Connection to {:?} closed: {:?}", peer_id, cause);
        break;
      }
      _ => {}
    }
  }

  Ok(())
}
