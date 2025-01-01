use std::{collections::HashMap, env::args, error::Error, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD};
use either::Either;
use env_logger::{Builder, Env};
use ethers::{
  prelude::Address,
  utils::{keccak256, to_checksum},
};
use libp2p::{
  Multiaddr, PeerId, StreamProtocol, SwarmBuilder, Transport,
  core::transport::upgrade::Version,
  futures::StreamExt,
  identify::{Behaviour as IdentifyBehavior, Config as IdentifyConfig, Event as IdentifyEvent},
  identity::{self, secp256k1::PublicKey as Secp256k1PublicKey},
  kad::{
    Behaviour as KadBehavior, Config as KadConfig, Event as KadEvent, RoutingUpdate,
    store::MemoryStore as KadInMemory,
  },
  noise,
  pnet::{PnetConfig, PreSharedKey},
  request_response::{
    Config as RequestResponseConfig, Event as RequestResponseEvent,
    Message as RequestResponseMessage, ProtocolSupport as RequestResponseProtocolSupport,
    cbor::Behaviour as RequestResponseBehavior,
  },
  swarm::SwarmEvent,
  tcp, yamux,
};
use log::{error, info, warn};

mod behavior;
mod message;

use behavior::{Behavior as AgentBehavior, Event as AgentEvent};
use message::{GreeRequest, GreetResponse};

/// Read the pre shared key file from the given ipfs directory
fn get_psk() -> Result<PreSharedKey, Box<dyn Error>> {
  let base64_key =
    std::env::var("PRIVITE_NET_KEY").map_err(|_| "PRIVITE_NET_KEY missing in .env")?;
  let bytes = STANDARD.decode(&base64_key)?;
  let key: [u8; 32] = bytes
    .try_into()
    .map_err(|_| "Decoded key must be 32 bytes long")?;
  Ok(PreSharedKey::new(key))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  dotenv::dotenv().ok();
  Builder::from_env(Env::default().default_filter_or("debug")).init();

  let psk = get_psk();
  let local_key = identity::Keypair::generate_secp256k1();

  let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
    .with_tokio()
    .with_other_transport(|key| {
      let noise_config = noise::Config::new(key).unwrap();
      let mut yamux_config = yamux::Config::default();
      yamux_config.set_max_num_streams(1024 * 1024);
      let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
      let maybe_encrypted = match psk {
        Ok(psk) => Either::Left(
          base_transport.and_then(move |socket, _| PnetConfig::new(psk).handshake(socket)),
        ),
        Err(_) => Either::Right(base_transport),
      };
      maybe_encrypted
        .upgrade(Version::V1Lazy)
        .authenticate(noise_config)
        .multiplex(yamux_config)
    })?
    .with_behaviour(|key| {
      let local_peer_id = PeerId::from(key.clone().public());
      info!("LocalPeerID: {local_peer_id}");

      let kad_config = KadConfig::new(StreamProtocol::new("/agent/connection/1.0.0"));
      let kad_memory = KadInMemory::new(local_peer_id);
      let kad = KadBehavior::with_config(local_peer_id, kad_memory, kad_config);

      let rr_config = RequestResponseConfig::default();
      let rr_protocol = StreamProtocol::new("/agent/message/1.0.0");
      let rr_behavior = RequestResponseBehavior::<GreeRequest, GreetResponse>::new(
        [(rr_protocol, RequestResponseProtocolSupport::Full)],
        rr_config,
      );

      let identify_config =
        IdentifyConfig::new("/agent/connection/1.0.0".to_string(), key.clone().public())
          .with_push_listen_addr_updates(true)
          .with_interval(Duration::from_secs(30));
      let identify = IdentifyBehavior::new(identify_config);
      AgentBehavior::new(kad, identify, rr_behavior)
    })?
    .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(30)))
    .build();

  swarm.behaviour_mut().set_server_mode();

  if let Some(addr) = args().nth(1) {
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let remote: Multiaddr = addr.parse()?;
    swarm.dial(remote)?;
    info!("Dialed to: {addr}");
  } else {
    info!("Act as bootstrap node");
    swarm.listen_on("/ip4/0.0.0.0/tcp/8000".parse()?)?;
  }

  let mut peers: HashMap<PeerId, Vec<Multiaddr>> = HashMap::new();
  loop {
    match swarm.select_next_some().await {
      SwarmEvent::NewListenAddr {
        listener_id,
        address,
      } => info!("NewListenAddr: {listener_id:?} | {address:?}"),
      SwarmEvent::ConnectionEstablished {
        peer_id,
        connection_id,
        endpoint,
        num_established,
        concurrent_dial_errors,
        established_in,
      } => info!(
        "ConnectionEstablished: {peer_id} | {connection_id} | {endpoint:?} | {num_established} | \
         {concurrent_dial_errors:?} | {established_in:?}"
      ),
      SwarmEvent::Dialing {
        peer_id,
        connection_id,
      } => info!("Dialing: {peer_id:?} | {connection_id}"),
      SwarmEvent::Behaviour(AgentEvent::Identify(event)) => match event {
        IdentifyEvent::Sent { peer_id, .. } => info!("IdentifyEvent:Sent: {peer_id}"),
        IdentifyEvent::Pushed { peer_id, info, .. } => {
          info!("IdentifyEvent:Pushed: {peer_id} | {info:?}")
        }
        IdentifyEvent::Received { peer_id, info, .. } => {
          info!("IdentifyEvent:Received: {peer_id} | {info:?}");
          if let Ok(libp2p_public_key) = info.public_key.clone().try_into_secp256k1() {
            if let Ok(libp2p_eth_address) = secpe256k1_publickey_to_eth_address(&libp2p_public_key)
            {
              info!("libp2p Ethereum Address: {}", libp2p_eth_address);
            }
          }

          peers.insert(peer_id, info.clone().listen_addrs);

          for addr in info.clone().listen_addrs {
            let agent_routing = swarm
              .behaviour_mut()
              .register_addr_kad(&peer_id, addr.clone());
            match agent_routing {
              RoutingUpdate::Failed => {
                error!("IdentifyReceived: Failed to register address to Kademlia")
              }
              RoutingUpdate::Pending => warn!("IdentifyReceived: Register address pending"),
              RoutingUpdate::Success => {
                info!("IdentifyReceived: {addr}: Success register address");
              }
            }

            let local_peer_id = local_key.public().to_peer_id();
            let message = GreeRequest {
              message: format!("Send message from: {local_peer_id}: Hello gaess"),
            };
            let request_id = swarm.behaviour_mut().send_message(&peer_id, message);
            info!("RequestID: {request_id}")
          }

          info!("Available peers: {peers:?}");
        }
        _ => {}
      },
      SwarmEvent::Behaviour(AgentEvent::RequestResponse(event)) => match event {
        RequestResponseEvent::Message { peer, message } => match message {
          RequestResponseMessage::Request {
            request_id,
            request,
            channel,
          } => {
            info!(
              "RequestResponseEvent::Message::Request -> PeerID: {peer} | RequestID: {request_id} \
               | RequestMessage: {request:?}"
            );
            let local_peer_id = local_key.public().to_peer_id();
            let response = GreetResponse {
              message: format!("Response from: {local_peer_id}: hello too").to_string(),
            };
            let result = swarm.behaviour_mut().send_response(channel, response);
            if result.is_err() {
              let err = result.unwrap_err();
              error!("Error sending response: {err:?}")
            } else {
              info!("Sending a message was success")
            }
          }
          RequestResponseMessage::Response {
            request_id,
            response,
          } => {
            info!(
              "RequestResponseEvent::Message::Response -> PeerID: {peer} | RequestID: \
               {request_id} | Response: {response:?}"
            )
          }
        },
        RequestResponseEvent::InboundFailure {
          peer,
          request_id,
          error,
        } => {
          warn!(
            "RequestResponseEvent::InboundFailure -> PeerID: {peer} | RequestID: {request_id} | \
             Error: {error}"
          )
        }
        RequestResponseEvent::ResponseSent { peer, request_id } => {
          info!("RequestResponseEvent::ResponseSent -> PeerID: {peer} | RequestID: {request_id}")
        }
        RequestResponseEvent::OutboundFailure {
          peer,
          request_id,
          error,
        } => {
          warn!(
            "RequestResponseEvent::OutboundFailure -> PeerID: {peer} | RequestID: {request_id} | \
             Error: {error}"
          )
        }
      },
      SwarmEvent::Behaviour(AgentEvent::Kad(event)) => match event {
        KadEvent::ModeChanged { new_mode } => info!("KadEvent:ModeChanged: {new_mode}"),
        KadEvent::RoutablePeer { peer, address } => {
          info!("KadEvent:RoutablePeer: {peer} | {address}")
        }
        KadEvent::PendingRoutablePeer { peer, address } => {
          info!("KadEvent:PendingRoutablePeer: {peer} | {address}")
        }
        KadEvent::InboundRequest { request } => info!("KadEvent:InboundRequest: {request:?}"),
        KadEvent::RoutingUpdated {
          peer,
          is_new_peer,
          addresses,
          bucket_range,
          old_peer,
        } => {
          info!(
            "KadEvent:RoutingUpdated: {peer} | IsNewPeer? {is_new_peer} | {addresses:?} | \
             {bucket_range:?} | OldPeer: {old_peer:?}"
          );
        }
        KadEvent::OutboundQueryProgressed {
          id,
          result,
          stats,
          step,
        } => {
          info!(
            "KadEvent:OutboundQueryProgressed: ID: {id:?} | Result: {result:?} | Stats: {stats:?} \
             | Step: {step:?}"
          )
        }
        _ => {}
      },
      _ => {}
    }
  }
}

pub fn secpe256k1_publickey_to_eth_address(
  pub_key: &Secp256k1PublicKey,
) -> Result<String, Box<dyn Error>> {
  let pub_key_bytes = pub_key.to_bytes_uncompressed();

  let hash = keccak256(&pub_key_bytes[1 ..]); // Skip the 0x04 prefix
  let address = Address::from_slice(&hash[12 ..]);

  Ok(to_checksum(&address, None).to_lowercase())
}
