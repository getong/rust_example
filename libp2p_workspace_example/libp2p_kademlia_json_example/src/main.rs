use std::{
  collections::{HashMap, hash_map::DefaultHasher},
  env::args,
  error::Error,
  hash::{Hash, Hasher},
  io::Write,
  time::Duration,
};

use env_logger::Builder;
use libp2p::{
  Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder,
  futures::StreamExt,
  gossipsub,
  identify::{Behaviour as IdentifyBehavior, Config as IdentifyConfig, Event as IdentifyEvent},
  identity::{self, Keypair},
  kad::{
    Behaviour as KadBehavior, Config as KadConfig, Event as KadEvent, RoutingUpdate,
    store::MemoryStore as KadInMemory,
  },
  noise::Config as NoiceConfig,
  request_response::{
    Config as RequestResponseConfig, Event as RequestResponseEvent,
    Message as RequestResponseMessage, ProtocolSupport as RequestResponseProtocolSupport,
    cbor::Behaviour as RequestResponseBehavior,
  },
  swarm::SwarmEvent,
  tcp::Config as TcpConfig,
  yamux::Config as YamuxConfig,
};
use log::{error, info, warn};

mod behavior;
mod message;
use behavior::{Behavior as AgentBehavior, Event as AgentEvent};
use message::{GreeRequest, GreetResponse, Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  Builder::new()
    .format(|buf, record| {
      writeln!(
        buf,
        "{}:{} {} [{}] - {}",
        record.file().unwrap_or("unknown"),
        record.line().unwrap_or(0),
        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
        record.level(),
        record.args()
      )
    })
    .filter(None, log::LevelFilter::Debug)
    .init();

  let local_key = identity::Keypair::generate_ed25519();

  let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
    .with_tokio()
    .with_tcp(TcpConfig::default(), NoiceConfig::new, YamuxConfig::default)?
    .with_quic()
    .with_dns()?
    .with_behaviour(|key| {
      let local_peer_id = PeerId::from(key.clone().public());
      info!("LocalPeerID: {local_peer_id}");

      let kad_config = KadConfig::new(StreamProtocol::new("/agent/connection/1.0.0"));

      let kad_memory = KadInMemory::new(local_peer_id);
      let kad = KadBehavior::with_config(local_peer_id, kad_memory, kad_config);

      let identify_config =
        IdentifyConfig::new("/agent/connection/1.0.0".to_string(), key.clone().public())
          .with_push_listen_addr_updates(true)
          .with_interval(Duration::from_secs(30));

      let rr_config = RequestResponseConfig::default();
      let rr_protocol = StreamProtocol::new("/agent/message/1.0.0");
      let rr_behavior = RequestResponseBehavior::<Vec<u8>, Vec<u8>>::new(
        [(rr_protocol, RequestResponseProtocolSupport::Full)],
        rr_config,
      );

      let identify = IdentifyBehavior::new(identify_config);

      let message_id_fn = |message: &gossipsub::Message| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        gossipsub::MessageId::from(s.finish().to_string())
      };
      // Set a custom gossipsub configuration
      let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
        .validation_mode(gossipsub::ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
        .message_id_fn(message_id_fn) // content-address messages. No two messages of the same content will be propagated.
        .build()
        .unwrap();

      // build a gossipsub network behaviour
      let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Signed(key.clone()),
        gossipsub_config,
      )
      .unwrap();

      AgentBehavior::new(kad, identify, rr_behavior, gossipsub)
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
    swarm.listen_on("/ip4/0.0.0.0/tcp/9000".parse()?)?;
  }

  let mut peers: HashMap<PeerId, Vec<Multiaddr>> = HashMap::new();
  loop {
    let event = swarm.select_next_some().await;
    handle_event(event, &mut peers, &mut swarm, local_key.clone()).await;
    println!("all peer are {:?}", swarm.behaviour_mut().known_peers());
  }
}

async fn handle_event(
  swarm_event: SwarmEvent<AgentEvent>,
  peers: &mut HashMap<PeerId, Vec<Multiaddr>>,
  swarm: &mut Swarm<AgentBehavior>,
  local_key: Keypair,
) {
  match swarm_event {
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
    SwarmEvent::Behaviour(AgentEvent::Gossipsub(event)) => info!("gossipsub: {event:?}"),
    SwarmEvent::Behaviour(AgentEvent::Identify(event)) => match event {
      IdentifyEvent::Sent { peer_id, .. } => info!("IdentifyEvent:Sent: {peer_id}"),
      IdentifyEvent::Pushed { peer_id, info, .. } => {
        info!("IdentifyEvent:Pushed: {peer_id} | {info:?}")
      }
      IdentifyEvent::Received { peer_id, info, .. } => {
        info!("IdentifyEvent:Received: {peer_id} | {info:?}");
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

          _ = swarm
            .behaviour_mut()
            .register_addr_kad(&peer_id, addr.clone());

          let local_peer_id = local_key.public().to_peer_id();
          let request = GreeRequest {
            message: format!("Send message from: {local_peer_id}: Hello gaess"),
          };
          let resquest_message = Message::GreeRequest(request);
          let request_id = swarm
            .behaviour_mut()
            .send_message(&peer_id, resquest_message);
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
          let parsed_request = Message::from_binary(&request).expect("Failed to decode request");
          match parsed_request {
            Message::GreeRequest(req) => {
              info!(
                "RequestResponseEvent::Message::Request -> PeerID: {peer} | RequestID: \
                 {request_id} | RequestMessage: {0:?}",
                req.message
              );
            }
            Message::AnotherMessage(msg) => {
              info!(
                "RequestResponseEvent::Message::Request -> PeerID: {peer} | RequestID: \
                 {request_id} | AnotherMessage: {0:?}",
                msg.info
              );
            }
            _ => {
              info!("Received unknown message type.");
            }
          }
          let local_peer_id = local_key.public().to_peer_id();
          let response = GreetResponse {
            message: format!("Response from: {local_peer_id}: hello too").to_string(),
          };
          let response_message = Message::GreetResponse(response);
          let result = swarm
            .behaviour_mut()
            .send_response(channel, response_message);
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
          let parsed_response = Message::from_binary(&response).expect("Failed to decode response");
          match parsed_response {
            Message::GreetResponse(res) => {
              info!(
                "RequestResponseEvent::Message::Response -> PeerID: {peer} | RequestID: \
                 {request_id} | Response: {0:?}",
                res.message
              )
            }
            _ => {
              info!("Received unknown response type.");
            }
          }
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
          "KadEvent:OutboundQueryProgressed: ID: {id:?} | Result: {result:?} | Stats: {stats:?} | \
           Step: {step:?}"
        )
      }
      _ => {}
    },
    _ => {}
  }
}
