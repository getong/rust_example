// # Step 1: Generate the private key in PEM format
// openssl ecparam -name secp256k1 -genkey -noout -out /tmp/private_key.pem
//
// # Step 2: Convert the PEM key to a raw hex format and save it to identity.txt
// openssl ec -in /tmp/private_key.pem -text -noout | grep priv -A 3 | tail -n +2 | tr -d
// '\n[:space:]:' > /tmp/identity.txt
//
// # Optionally, remove the PEM file
// rm /tmp/private_key.pem
use std::{fs, time::Duration};

use futures::StreamExt;
use libp2p::{
  identity::{self, Keypair},
  kad, noise,
  request_response::{self, ProtocolSupport},
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux, Multiaddr, PeerId, StreamProtocol,
};
use serde::{Deserialize, Serialize};

#[derive(NetworkBehaviour)]
struct Behaviour {
  request_response: request_response::cbor::Behaviour<FileRequest, FileResponse>,
  kademlia: kad::Behaviour<kad::store::MemoryStore>,
}

// Simple file exchange protocol
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FileRequest(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileResponse(Vec<u8>);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let private_key_str = fs::read_to_string("/tmp/identity.txt")?.trim().to_string();

  if private_key_str.is_empty() {
    return Err(format!("Private key is empty in file: /tmp/identity.txt").into());
  }

  // Decode the hex string into bytes
  let private_key_bytes = hex::decode(private_key_str)?;

  // Check if the length of the private key is valid
  if private_key_bytes.len() != 32 {
    return Err("Private key must be exactly 32 bytes".into());
  }

  // Create a libp2p Keypair from the secp256k1 private key
  let secret_key = identity::secp256k1::SecretKey::try_from_bytes(private_key_bytes)?;
  let local_key: Keypair = identity::secp256k1::Keypair::from(secret_key).into();
  let local_peer_id = PeerId::from(local_key.public());
  println!("Local peer id: {:?}", local_peer_id);

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

  // Start listening on the given multiaddress
  let listen_addr: Multiaddr = "/ip4/0.0.0.0/tcp/4001".parse()?; // Use a specific port like 4001
  println!("listen_addr： {:?}", listen_addr);
  swarm.listen_on(listen_addr)?;

  swarm
    .behaviour_mut()
    .kademlia
    .set_mode(Some(kad::Mode::Server));

  loop {
    match swarm.next().await.unwrap() {
      SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
        request_response::Event::Message {
          message: request_response::Message::Request {
            request, channel, ..
          },
          ..
        },
      )) => {
        println!("Received request: {:?}", request);
        let response = FileResponse(b"pong".to_vec());
        swarm
          .behaviour_mut()
          .request_response
          .send_response(channel, response)
          .unwrap();
      }
      SwarmEvent::NewListenAddr { address, .. } => {
        println!("Listening on {:?}", address);
      }
      _ => {}
    }
  }
}
