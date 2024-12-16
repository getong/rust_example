// pub fn add(left: u64, right: u64) -> u64 {
//   left + right
// }

// #[cfg(test)]
// mod tests {
//   use super::*;

//   #[test]
//   fn it_works() {
//     let result = add(2, 2);
//     assert_eq!(result, 4);
//   }
// }
use std::error::Error;

use futures::StreamExt;
use libp2p::{
  Multiaddr, PeerId, Transport,
  core::upgrade,
  floodsub::{self, Floodsub, FloodsubEvent},
  identity, mdns, noise,
  pnet::{PnetConfig, PreSharedKey},
  request_response::ProtocolSupport,
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp,
};
use tokio::io::{self, AsyncBufReadExt};

pub async fn load_keys_or_generate() -> Result<identity::Keypair, Box<dyn Error>> {
  let file = tokio::fs::File::open("keys.binary").await;
  if let Ok(mut file) = file {
    let mut buffer = Vec::new();
    io::AsyncReadExt::read_to_end(&mut file, &mut buffer).await?;
    let keypair = identity::Keypair::from_protobuf_encoding(&buffer)?;
    Ok(keypair)
  } else {
    let keypair = identity::Keypair::generate_ed25519();
    let keypair_bytes = keypair.to_protobuf_encoding()?;
    let mut file = tokio::fs::File::create("keys.binary").await?;
    io::AsyncWriteExt::write_all(&mut file, &keypair_bytes).await?;
    Ok(keypair)
  }
}

#[derive(Clone)]
struct Codec {}

#[derive(Clone)]
struct ApeiroProtocolName {}

impl AsRef<str> for ApeiroProtocolName {
  fn as_ref(&self) -> &str {
    "/apeiro/1.0"
  }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProtocolRequest(RemoteDEngineCmd);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProtocolResponse(String);

use async_trait::async_trait;
use futures::prelude::*;

use crate::{DEngine, dengine::RemoteDEngineCmd};

#[async_trait]
impl libp2p::request_response::Codec for Codec {
  type Protocol = ApeiroProtocolName;

  type Request = ProtocolRequest;

  type Response = ProtocolResponse;

  async fn read_request<T>(
    &mut self,
    _: &ApeiroProtocolName,
    io: &mut T,
  ) -> io::Result<Self::Request>
  where
    T: AsyncRead + Unpin + Send,
  {
    let vec = read_length_prefixed(io, 1_000_000).await?;

    if vec.is_empty() {
      return Err(io::ErrorKind::UnexpectedEof.into());
    }
    Ok(ProtocolRequest(serde_json::from_slice(&vec).unwrap()))
  }
  async fn read_response<T>(
    &mut self,
    _: &ApeiroProtocolName,
    io: &mut T,
  ) -> io::Result<Self::Response>
  where
    T: AsyncRead + Unpin + Send,
  {
    let vec = read_length_prefixed(io, 1_000_000).await?;

    if vec.is_empty() {
      return Err(io::ErrorKind::UnexpectedEof.into());
    }

    Ok(ProtocolResponse(String::from_utf8(vec).unwrap()))
  }

  async fn write_request<T>(
    &mut self,
    _: &ApeiroProtocolName,
    io: &mut T,
    ProtocolRequest(data): ProtocolRequest,
  ) -> io::Result<()>
  where
    T: AsyncWrite + Unpin + Send,
  {
    let data = serde_json::to_vec(&data).unwrap();
    write_length_prefixed(io, data).await?;
    io.close().await?;

    Ok(())
  }

  async fn write_response<T>(
    &mut self,
    _: &ApeiroProtocolName,
    io: &mut T,
    ProtocolResponse(data): ProtocolResponse,
  ) -> io::Result<()>
  where
    T: AsyncWrite + Unpin + Send,
  {
    write_length_prefixed(io, data).await?;
    io.close().await?;

    Ok(())
  }
}

pub async fn start_p2p(
  dengine: DEngine,
  addrs_to_dial: Vec<String>,
) -> Result<tokio::sync::mpsc::Sender<RemoteDEngineCmd>, Box<dyn Error>> {
  // Create a random PeerId
  let id_keys = load_keys_or_generate().await?;
  let peer_id = PeerId::from(id_keys.public());
  println!("Local peer id: {peer_id:?}");
  println!("pubi {:?}", id_keys.public());

  let base_transport = tcp::async_io::Transport::new(tcp::Config::default().nodelay(true));

  let (relay_transport, relay_client) =
    libp2p::relay::client::Client::new_transport_and_behaviour(peer_id);

  let base_transport = libp2p::core::transport::OrTransport::new(relay_transport, base_transport);

  let psk = Some(PreSharedKey::new(*b"apeiroasdfasdhfkjhasdlfjhadsjlff"));

  let maybe_encrypted = match psk {
    Some(psk) => EitherTransport::Left(
      base_transport.and_then(move |socket, _| PnetConfig::new(psk).handshake(socket)),
    ),
    None => EitherTransport::Right(base_transport),
  };

  let noise = noise::Config::new(&id_keys).unwrap();

  let transport = maybe_encrypted
    .upgrade(upgrade::Version::V1)
    .authenticate(noise)
    .multiplex(mplex::MplexConfig::new())
    .boxed();

  // Create a Floodsub topic
  let floodsub_topic = floodsub::Topic::new("chat");

  let req_resp = libp2p::request_response::RequestResponse::new(
    Codec {},
    std::iter::once((ApeiroProtocolName {}, ProtocolSupport::Full)),
    libp2p::request_response::RequestResponseConfig::default(),
  );
  // We create a custom  behaviour that combines floodsub and mDNS.
  // The derive generates a delegating `NetworkBehaviour` impl.
  #[derive(NetworkBehaviour)]
  #[behaviour(out_event = "MyBehaviourEvent")]
  struct MyBehaviour {
    floodsub: Floodsub,
    mdns: mdns::tokio::Behaviour,
    ping: libp2p::ping::Behaviour,
    request_response: libp2p::request_response::cbor::Behaviour<ProtocolRequest, ProtocolResponse>,
    relay: libp2p::relay::client::Client,
  }

  #[allow(clippy::large_enum_variant)]
  #[derive(Debug)]
  enum MyBehaviourEvent {
    Floodsub(FloodsubEvent),
    Mdns(mdns::Event),
    Ping(libp2p::ping::Event),
    ReqResp(RequestResponseEvent<ProtocolRequest, ProtocolResponse>),
    Relay(libp2p::relay::client::Event),
  }

  impl From<libp2p::relay::client::Event> for MyBehaviourEvent {
    fn from(event: libp2p::relay::client::Event) -> Self {
      MyBehaviourEvent::Relay(event)
    }
  }

  impl From<RequestResponseEvent<ProtocolRequest, ProtocolResponse>> for MyBehaviourEvent {
    fn from(event: RequestResponseEvent<ProtocolRequest, ProtocolResponse>) -> Self {
      MyBehaviourEvent::ReqResp(event)
    }
  }

  impl From<libp2p::ping::Event> for MyBehaviourEvent {
    fn from(event: libp2p::ping::Event) -> Self {
      MyBehaviourEvent::Ping(event)
    }
  }

  impl From<FloodsubEvent> for MyBehaviourEvent {
    fn from(event: FloodsubEvent) -> Self {
      MyBehaviourEvent::Floodsub(event)
    }
  }

  impl From<mdns::Event> for MyBehaviourEvent {
    fn from(event: mdns::Event) -> Self {
      MyBehaviourEvent::Mdns(event)
    }
  }

  // Create a Swarm to manage peers and events.
  let mdns_behaviour = mdns::tokio::Behaviour::new(Default::default())?;
  let mut behaviour = MyBehaviour {
    floodsub: Floodsub::new(peer_id),
    mdns: mdns_behaviour,
    ping: libp2p::ping::Behaviour::new(libp2p::ping::Config::new()),
    request_response: req_resp,
    relay: relay_client,
  };

  behaviour.floodsub.subscribe(floodsub_topic.clone());

  let mut swarm = libp2p::Swarm::with_tokio_executor(transport, behaviour, peer_id);

  for to_dial in addrs_to_dial {
    let addr: Multiaddr = to_dial.parse()?;
    swarm.dial(addr)?;
    println!("Dialed {to_dial:?}");
  }

  // Read full lines from stdin
  let mut stdin = io::BufReader::new(io::stdin()).lines();

  // Listen on all interfaces and whatever port the OS assigns
  swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

  use tokio::time::{self, Duration};

  let mut tick = time::interval(Duration::from_millis(1000));

  let (sender, mut receiver) = tokio::sync::mpsc::channel::<RemoteDEngineCmd>(100);
  // Kick it off
  tokio::task::spawn(async move {
    loop {
      tokio::select! {
          _ = tick.tick() => {
              println!("tick");
          }
          internal_event = receiver.recv() => {
              match internal_event {
                  Some(event) => {
                      swarm.behaviour_mut().request_response.send_request(
                          &event.peer_id.clone().parse().unwrap(),
                          ProtocolRequest(event),
                      );
                  }
                  None => {
                  }
              }
          }
          line = stdin.next_line() => {
              println!("{:?}", swarm.connected_peers().collect::<Vec<&PeerId>>());
              println!("{:?}", swarm.network_info());
              let line = line.unwrap().expect("stdin closed");
              if line.starts_with("/connect ") {
                  let split = line.split(" ").collect::<Vec<&str>>();
                  let addr: Multiaddr = split[1].parse().unwrap();
                  swarm.dial(addr).unwrap();
              } else if line.starts_with("/send ") {
                  let split = line.split(" ").collect::<Vec<&str>>();
                  let peer_id = split[1];
                  let _msg = split[2];
                  swarm.behaviour_mut().request_response.send_request(
                      &peer_id.parse().unwrap(),
                      ProtocolRequest(
                          RemoteDEngineCmd {
                              peer_id: peer_id.to_string(),
                              cmd: crate::dengine::DEngineCmd::Broadcast(
                                  "test".to_string(),
                                  "test2".to_string(),
                                  crate::dengine::ProcEvent::None,
                              ),
                          }
                  ));
              } else {
                  swarm.behaviour_mut().floodsub.publish(floodsub_topic.clone(), line.as_bytes());
              }
          }
          event = swarm.select_next_some() => {
              match event {
                  SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                      println!("Connection with {peer_id:?} established on {endpoint:?}");
                      swarm.behaviour_mut().floodsub.add_node_to_partial_view(peer_id);
                  }
                  SwarmEvent::NewListenAddr { address, .. } => {
                      println!("Listening on {address:?}");
                  }
                  SwarmEvent::Behaviour(MyBehaviourEvent::ReqResp(event)) => {
                      println!("event: {:?}", event);
                      if let RequestResponseEvent::Message { peer, message } = event {
                          if let RequestResponseMessage::Request { request_id, request, channel } = message {
                              println!("Received request from {peer:?} {request:?} {request_id:?}");
                              let response = ProtocolResponse("hello".to_string());
                              dengine.send(request.0.cmd).await.unwrap();
                              swarm.behaviour_mut().request_response.send_response(channel, response).unwrap();
                          }
                      }
                  }
                  SwarmEvent::Behaviour(MyBehaviourEvent::Floodsub(FloodsubEvent::Message(message))) => {
                      println!(
                              "Received: '{:?}' from {:?}",
                              String::from_utf8_lossy(&message.data),
                              message.source
                          );
                  }
                  SwarmEvent::Behaviour(MyBehaviourEvent::Mdns(event)) => {
                      match event {
                          mdns::Event::Discovered(list) => {
                              for (peer, addr) in list {
                                  println!("discovered {peer} {addr}");
                                  swarm.dial(addr.clone()).unwrap();
                              }
                          }
                          mdns::Event::Expired(list) => {
                              for (peer, addr) in list {
                                  println!("expired {peer} {addr}");
                                  if !swarm.behaviour().mdns.has_node(&peer) {
                                      // swarm.behaviour_mut().floodsub.remove_node_from_partial_view(&peer);
                                  }
                              }
                          }
                      }
                  }
                  e => {
                      println!("event {:?}", e);
                  }
              }
          }
      }
    }
  });
  Ok(sender)
}

// copy from https://github.com/ianatha/apeiro
