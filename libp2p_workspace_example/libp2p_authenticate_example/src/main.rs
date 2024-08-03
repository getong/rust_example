use either::Either;
use env_logger::{Builder, Env};
use libp2p::{
  core::transport::upgrade::Version,
  futures::StreamExt,
  identify::{Behaviour as IdentifyBehavior, Config as IdentifyConfig, Event as IdentifyEvent},
  identity,
  kad::{
    store::MemoryStore as KadInMemory, Behaviour as KadBehavior, Config as KadConfig,
    Event as KadEvent, RoutingUpdate,
  },
  noise::Config as NoiseConfig,
  pnet::{PnetConfig, PreSharedKey},
  request_response::{
    cbor::Behaviour as RequestResponseBehavior, Config as RequestResponseConfig,
    Event as RequestResponseEvent, Message as RequestResponseMessage,
    ProtocolSupport as RequestResponseProtocolSupport,
  },
  swarm::SwarmEvent,
  tcp,
  yamux::Config as YamuxConfig,
  Multiaddr,
  PeerId,
  StreamProtocol,
  SwarmBuilder,
  Transport,
};
use log::{error, info, warn};
use std::{
  collections::HashMap, env, env::args, error::Error, fs, path::Path, str::FromStr, time::Duration,
};

mod behavior;
mod message;

use behavior::{Behavior as AgentBehavior, Event as AgentEvent};
use message::{GreeRequest, GreetResponse};

/// Get the current ipfs repo path, either from the IPFS_PATH environment variable or
/// from the default $HOME/.ipfs
fn get_ipfs_path() -> Box<Path> {
  env::var("IPFS_PATH")
    .map(|ipfs_path| Path::new(&ipfs_path).into())
    .unwrap_or_else(|_| {
      env::var("HOME")
        .map(|home| Path::new(&home).join(".ipfs"))
        .expect("could not determine home directory")
        .into()
    })
}

/// Read the pre shared key file from the given ipfs directory
fn get_psk(path: &Path) -> std::io::Result<Option<String>> {
  let swarm_key_file = path.join("swarm.key");
  match fs::read_to_string(swarm_key_file) {
    Ok(text) => Ok(Some(text)),
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
    Err(e) => Err(e),
  }
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  Builder::from_env(Env::default().default_filter_or("debug")).init();

  let local_key = identity::Keypair::generate_ed25519();
  let ipfs_path = get_ipfs_path();
  println!("using IPFS_PATH {ipfs_path:?}");
  let psk: Option<PreSharedKey> = get_psk(&ipfs_path)?
    .map(|text| PreSharedKey::from_str(&text))
    .transpose()?;

  if let Some(psk) = psk {
    println!("using swarm key with fingerprint: {}", psk.fingerprint());
  }

  let mut swarm = SwarmBuilder::with_existing_identity(local_key.clone())
    .with_tokio()
    .with_other_transport(|key| {
      let noise_config = NoiseConfig::new(key).unwrap();
      let yamux_config = YamuxConfig::default();

      let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
      let maybe_encrypted = match psk {
        Some(psk) => Either::Left(
          base_transport.and_then(move |socket, _| PnetConfig::new(psk).handshake(socket)),
        ),
        None => Either::Right(base_transport),
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

      let identify_config =
        IdentifyConfig::new("/agent/connection/1.0.0".to_string(), key.clone().public())
          .with_push_listen_addr_updates(true)
          .with_interval(Duration::from_secs(30));

      let rr_config = RequestResponseConfig::default();
      let rr_protocol = StreamProtocol::new("/agent/message/1.0.0");
      let rr_behavior = RequestResponseBehavior::<GreeRequest, GreetResponse>::new(
        [(rr_protocol, RequestResponseProtocolSupport::Full)],
        rr_config,
      );

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
            SwarmEvent::NewListenAddr { listener_id, address } => info!("NewListenAddr: {listener_id:?} | {address:?}"),
            SwarmEvent::ConnectionEstablished {
                peer_id,
                connection_id,
                endpoint,
                num_established,
                concurrent_dial_errors,
                established_in } => info!("ConnectionEstablished: {peer_id} | {connection_id} | {endpoint:?} | {num_established} | {concurrent_dial_errors:?} | {established_in:?}"),
            SwarmEvent::Dialing { peer_id, connection_id } => info!("Dialing: {peer_id:?} | {connection_id}"),
            SwarmEvent::Behaviour(AgentEvent::Identify(event)) => match event {
                IdentifyEvent::Sent { peer_id, .. } => info!("IdentifyEvent:Sent: {peer_id}"),
                IdentifyEvent::Pushed { peer_id, info, .. } => info!("IdentifyEvent:Pushed: {peer_id} | {info:?}"),
                IdentifyEvent::Received { peer_id, info, .. } => {
                    info!("IdentifyEvent:Received: {peer_id} | {info:?}");
                    peers.insert(peer_id, info.clone().listen_addrs);

                    for addr in info.clone().listen_addrs {
                        let agent_routing = swarm.behaviour_mut().register_addr_kad(&peer_id, addr.clone());
                        match agent_routing {
                            RoutingUpdate::Failed => error!("IdentifyReceived: Failed to register address to Kademlia"),
                            RoutingUpdate::Pending => warn!("IdentifyReceived: Register address pending"),
                            RoutingUpdate::Success => {
                                info!("IdentifyReceived: {addr}: Success register address");
                            }
                        }

                        _ = swarm.behaviour_mut().register_addr_kad(&peer_id, addr.clone());

                        let local_peer_id = local_key.public().to_peer_id();
                        let message = GreeRequest{ message: format!("Send message from: {local_peer_id}: Hello gaess") };
                        let request_id = swarm.behaviour_mut().send_message(&peer_id, message);
                        info!("RequestID: {request_id}")
                    }

                    info!("Available peers: {peers:?}");
                },
                _ => {}
            },
            SwarmEvent::Behaviour(AgentEvent::RequestResponse(event)) => match event {
                RequestResponseEvent::Message { peer, message } => {
                    match message {
                        RequestResponseMessage::Request { request_id, request, channel} => {
                            info!("RequestResponseEvent::Message::Request -> PeerID: {peer} | RequestID: {request_id} | RequestMessage: {request:?}");
                            let local_peer_id = local_key.public().to_peer_id();
                            let response = GreetResponse{ message: format!("Response from: {local_peer_id}: hello too").to_string() };
                            let result = swarm.behaviour_mut().send_response(channel, response);
                            if result.is_err() {
                                let err = result.unwrap_err();
                                error!("Error sending response: {err:?}")
                            } else {
                                info!("Sending a message was success")
                            }
                        },
                        RequestResponseMessage::Response { request_id, response } => {
                            info!("RequestResponseEvent::Message::Response -> PeerID: {peer} | RequestID: {request_id} | Response: {response:?}")
                        }
                    }
                },
                RequestResponseEvent::InboundFailure { peer, request_id, error } => {
                    warn!("RequestResponseEvent::InboundFailure -> PeerID: {peer} | RequestID: {request_id} | Error: {error}")
                },
                RequestResponseEvent::ResponseSent { peer, request_id } => {
                    info!("RequestResponseEvent::ResponseSent -> PeerID: {peer} | RequestID: {request_id}")
                },
                RequestResponseEvent::OutboundFailure { peer, request_id, error } => {
                    warn!("RequestResponseEvent::OutboundFailure -> PeerID: {peer} | RequestID: {request_id} | Error: {error}")
                }
            },
            SwarmEvent::Behaviour(AgentEvent::Kad(event)) => match event {
                KadEvent::ModeChanged { new_mode } => info!("KadEvent:ModeChanged: {new_mode}"),
                KadEvent::RoutablePeer { peer, address } => info!("KadEvent:RoutablePeer: {peer} | {address}"),
                KadEvent::PendingRoutablePeer { peer, address } => info!("KadEvent:PendingRoutablePeer: {peer} | {address}"),
                KadEvent::InboundRequest { request } => info!("KadEvent:InboundRequest: {request:?}"),
                KadEvent::RoutingUpdated {
                    peer,
                    is_new_peer,
                    addresses,
                    bucket_range,
                    old_peer } => {
                        info!("KadEvent:RoutingUpdated: {peer} | IsNewPeer? {is_new_peer} | {addresses:?} | {bucket_range:?} | OldPeer: {old_peer:?}");
                    },
                KadEvent::OutboundQueryProgressed {
                    id,
                    result,
                    stats,
                    step } => {

                    info!("KadEvent:OutboundQueryProgressed: ID: {id:?} | Result: {result:?} | Stats: {stats:?} | Step: {step:?}")
                },
                _ => {}
            }
            _ => {}
        }
  }
}
