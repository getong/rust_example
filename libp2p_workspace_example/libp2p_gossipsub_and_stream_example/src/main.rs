use std::{
  collections::HashMap, env, env::args, error::Error, fs, path::Path, str::FromStr, time::Duration,
};

use either::Either;
use env_logger::{Builder, Env};
use libp2p::{
  core::transport::upgrade::Version,
  futures::StreamExt,
  gossipsub,
  identify::{Behaviour as IdentifyBehavior, Config as IdentifyConfig, Event as IdentifyEvent},
  identity::{self, Keypair},
  kad::{
    store::MemoryStore as KadInMemory, Behaviour as KadBehavior, Config as KadConfig,
    Event as KadEvent, RoutingUpdate,
  },
  mdns,
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
  Multiaddr, PeerId, StreamProtocol, Swarm, SwarmBuilder, Transport,
};
use libp2p_stream::Behaviour as StreamBehaviour;
use log::{error, info, warn};
use tokio::{
  io::{self, AsyncBufReadExt},
  signal::unix::{signal, Signal, SignalKind},
  sync::mpsc::{self, Receiver, Sender},
  task::JoinHandle,
};

mod behavior;
mod message;

use behavior::{Behavior as AgentBehavior, BehaviorEvent};
use message::{GreeRequest, GreetResponse};

const AUDIO_STREAM_PROTOCOL: StreamProtocol = StreamProtocol::new("/audio");

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
  let mut swarm = generate_swarm(local_key.clone(), psk).unwrap();

  swarm.behaviour_mut().set_server_mode();

  let mut incoming_streams = swarm
    .behaviour()
    .stream
    .new_control()
    .accept(AUDIO_STREAM_PROTOCOL)?;

  tokio::spawn(async move {
    while let Some((their_peer_id, output_stream)) = incoming_streams.next().await {
      println!(
        "their_peer_id:{:?}, output_stream:{:?}",
        their_peer_id, output_stream
      );
    }
  });

  _ = dial_swarm_server_or_bootstrap(&mut swarm).await;

  let (tx, rx) = mpsc::channel(100);
  get_stdin_input_message(tx).await;
  let handler = handle_swarm_and_shutdown(local_key, swarm, rx).await?;
  _ = handler.await;

  Ok(())
}

async fn dial_swarm_server_or_bootstrap(
  swarm: &mut Swarm<AgentBehavior>,
) -> Result<(), Box<dyn Error>> {
  if let Some(addr) = args().nth(1) {
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let remote: Multiaddr = addr.parse()?;
    swarm.dial(remote)?;
    info!("Dialed to: {addr}");
  } else {
    info!("Act as bootstrap node");
    swarm.listen_on("/ip4/0.0.0.0/tcp/8000".parse()?)?;
  }
  Ok(())
}

async fn get_stdin_input_message(tx: Sender<String>) {
  tokio::spawn(async move {
    let mut stdin = io::BufReader::new(io::stdin()).lines();
    while let Ok(Some(line)) = stdin.next_line().await {
      _ = tx.try_send(line)
    }
  });
}

async fn handle_swarm_and_shutdown(
  local_key: Keypair,
  mut swarm: Swarm<AgentBehavior>,
  mut rx: Receiver<String>,
) -> Result<JoinHandle<()>, Box<dyn Error>> {
  let mut peers: HashMap<PeerId, Vec<Multiaddr>> = HashMap::new();
  let mut sig_int = signal(SignalKind::interrupt())?;
  let mut sig_term = signal(SignalKind::terminate())?;

  Ok(tokio::spawn(async move {
    loop {
      tokio::select! {
        _ = handle_swarm_event(local_key.clone(), &mut swarm, &mut peers) => {},

        _ = recv_terminal_signal(&mut sig_int, &mut sig_term) => {
          println!("recv terminal signal");
          break;
        }

        Some(line) = rx.recv() => {
          // println!("recv line is {}", line);
          for local_peer_id in peers.keys(){
            let message = GreeRequest{ message: format!("Send message from stdio: {local_peer_id}: {line}") };
            _ = swarm.behaviour_mut().send_message(local_peer_id, message);
          }
        }
      }
    }
  }))
}

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

async fn handle_swarm_event(
  local_key: Keypair,
  swarm: &mut Swarm<AgentBehavior>,
  peers: &mut HashMap<PeerId, Vec<Multiaddr>>,
) {
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
    SwarmEvent::Behaviour(BehaviorEvent::Identify(event)) => {
      handle_identify_event(event, local_key, swarm, peers).await
    }
    SwarmEvent::Behaviour(BehaviorEvent::Rr(event)) => {
      handle_requestresponse_event(event, local_key, swarm).await
    }
    SwarmEvent::Behaviour(BehaviorEvent::Kad(event)) => handle_kad_event(event).await,
    SwarmEvent::Behaviour(BehaviorEvent::Gossipsub(event)) => handle_gossipsub_event(event).await,
    SwarmEvent::Behaviour(BehaviorEvent::Mdns(event)) => handle_mdns_event(event).await,
    _event => {
      println!("Unhandled swarm event: {:?}", _event);
    }
  }
}

async fn handle_identify_event(
  event: IdentifyEvent,
  local_key: Keypair,
  swarm: &mut Swarm<AgentBehavior>,
  peers: &mut HashMap<PeerId, Vec<Multiaddr>>,
) {
  match event {
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
        let message = GreeRequest {
          message: format!("Send message from: {local_peer_id}: Hello gaess"),
        };
        let request_id = swarm.behaviour_mut().send_message(&peer_id, message);
        info!("RequestID: {request_id}")
      }

      info!("Available peers: {peers:?}");
    }
    _ => {}
  }
}

async fn handle_requestresponse_event(
  event: RequestResponseEvent<GreeRequest, GreetResponse>,
  local_key: Keypair,
  swarm: &mut Swarm<AgentBehavior>,
) {
  match event {
    RequestResponseEvent::Message { peer, message, .. } => match message {
      RequestResponseMessage::Request {
        request_id,
        request,
        channel,
      } => {
        info!(
          "RequestResponseEvent::Message::Request -> PeerID: {peer} | RequestID: {request_id} | \
           RequestMessage: {request:?}"
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
          "RequestResponseEvent::Message::Response -> PeerID: {peer} | RequestID: {request_id} | \
           Response: {response:?}"
        )
      }
    },
    RequestResponseEvent::InboundFailure {
      peer,
      request_id,
      error,
      ..
    } => {
      warn!(
        "RequestResponseEvent::InboundFailure -> PeerID: {peer} | RequestID: {request_id} | \
         Error: {error}"
      )
    }
    RequestResponseEvent::ResponseSent {
      peer, request_id, ..
    } => {
      info!("RequestResponseEvent::ResponseSent -> PeerID: {peer} | RequestID: {request_id}")
    }
    RequestResponseEvent::OutboundFailure {
      peer,
      request_id,
      error,
      ..
    } => {
      warn!(
        "RequestResponseEvent::OutboundFailure -> PeerID: {peer} | RequestID: {request_id} | \
         Error: {error}"
      )
    }
  }
}

async fn handle_kad_event(event: KadEvent) {
  match event {
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
  }
}

async fn handle_gossipsub_event(event: gossipsub::Event) {
  println!("gossipsub event is {:?}", event);
}

async fn handle_mdns_event(event: mdns::Event) {
  println!("mdns event is {:?}", event);
}

fn generate_swarm(
  local_key: Keypair,
  psk: Option<PreSharedKey>,
) -> Result<Swarm<AgentBehavior>, Box<dyn Error>> {
  Ok(
    SwarmBuilder::with_existing_identity(local_key)
      .with_tokio()
      .with_tcp(
        tcp::Config::default().nodelay(true),
        NoiseConfig::new,
        YamuxConfig::default,
      )?
      .with_quic()
      .with_other_transport(|k| {
        let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
        let maybe_encrypted = match psk {
          Some(psk) => Either::Left(
            base_transport.and_then(move |socket, _| PnetConfig::new(psk).handshake(socket)),
          ),
          None => Either::Right(base_transport),
        };
        maybe_encrypted
          .upgrade(Version::V1Lazy)
          .authenticate(NoiseConfig::new(k).unwrap())
          .multiplex(YamuxConfig::default())
      })?
      .with_other_transport(|k| {
        let base_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true));
        let maybe_encrypted = match psk {
          Some(psk) => Either::Left(
            base_transport.and_then(move |socket, _| PnetConfig::new(psk).handshake(socket)),
          ),
          None => Either::Right(base_transport),
        };
        maybe_encrypted
          .upgrade(Version::V1)
          .authenticate(NoiseConfig::new(k).unwrap())
          .multiplex(YamuxConfig::default())
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

        let gossipsub_config = gossipsub::ConfigBuilder::default().build().unwrap();
        let gossipsub = gossipsub::Behaviour::new(
          gossipsub::MessageAuthenticity::Signed(key.clone()),
          gossipsub_config,
        )
        .unwrap();

        let mdns = mdns::tokio::Behaviour::new(
          mdns::Config {
            ttl: Duration::from_secs(5),
            query_interval: Duration::from_secs(1),
            ..Default::default()
          },
          key.public().to_peer_id(),
        )
        .unwrap();

        let stream = StreamBehaviour::new();

        AgentBehavior::new(kad, identify, rr_behavior, gossipsub, mdns, stream)
      })?
      .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(30)))
      .build(),
  )
}

async fn recv_terminal_signal(sig_int: &mut Signal, sig_term: &mut Signal) {
  tokio::select! {
    _ = sig_int.recv() => {}
    _ = sig_term.recv() => {}
  }
}
