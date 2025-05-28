use std::{
  collections::{HashMap, HashSet},
  error::Error,
  fmt,
};

use futures::prelude::*;
use libp2p::{
  PeerId, Swarm, SwarmBuilder,
  core::Multiaddr,
  identity, kad,
  mdns::{self, tokio::Behaviour as MdnsBehaviour},
  noise,
  request_response::{self, ProtocolSupport, ResponseChannel},
  swarm::{NetworkBehaviour, SwarmEvent},
  tcp, yamux,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot as tokio_oneshot};

#[derive(Clone)]
pub(crate) struct Client {
  sender: mpsc::UnboundedSender<Command>,
}

impl Client {
  pub(crate) async fn start_listening(
    &mut self,
    addr: Multiaddr,
  ) -> Result<(), Box<dyn Error + Send>> {
    let (sender, receiver) = tokio_oneshot::channel();
    self
      .sender
      .send(Command::StartListening { addr, sender })
      .expect("Command receiver not to be dropped.");
    receiver.await.expect("Sender not to be dropped.")
  }

  pub(crate) async fn dial(
    &mut self,
    peer_id: PeerId,
    peer_addr: Multiaddr,
  ) -> Result<(), Box<dyn Error + Send>> {
    let (sender, receiver) = tokio_oneshot::channel();
    self
      .sender
      .send(Command::Dial {
        peer_id,
        peer_addr,
        sender,
      })
      .expect("Command receiver not to be dropped.");
    receiver.await.expect("Sender not to be dropped.")
  }

  pub(crate) async fn start_providing(&mut self, file_name: String) {
    let (sender, receiver) = tokio_oneshot::channel();
    self
      .sender
      .send(Command::StartProviding { file_name, sender })
      .expect("Command receiver not to be dropped.");
    receiver.await.expect("Sender not to be dropped.");
  }

  pub(crate) async fn get_providers(&mut self, file_name: String) -> HashSet<PeerId> {
    let (sender, receiver) = tokio_oneshot::channel();
    self
      .sender
      .send(Command::GetProviders { file_name, sender })
      .expect("Command receiver not to be dropped.");
    receiver.await.expect("Sender not to be dropped.")
  }

  pub(crate) async fn request_file(
    &mut self,
    peer: PeerId,
    file_name: String,
  ) -> Result<Vec<u8>, Box<dyn Error + Send>> {
    let (sender, receiver) = tokio_oneshot::channel();
    self
      .sender
      .send(Command::RequestFile {
        file_name,
        peer,
        sender,
      })
      .expect("Command receiver not to be dropped.");
    receiver.await.expect("Sender not to be dropped.")
  }

  pub(crate) async fn respond_file(
    &mut self,
    file: Vec<u8>,
    channel: ResponseChannel<FileResponse>,
  ) {
    self
      .sender
      .send(Command::RespondFile { file, channel })
      .expect("Command receiver not to be dropped.");
  }
}

pub(crate) struct EventLoop {
  swarm: Swarm<ComposedBehaviour>,
  command_receiver: mpsc::UnboundedReceiver<Command>,
  event_sender: mpsc::UnboundedSender<Event>,
  pending_dial: HashMap<PeerId, tokio_oneshot::Sender<Result<(), Box<dyn Error + Send>>>>,
  pending_start_providing: HashMap<kad::QueryId, tokio_oneshot::Sender<()>>,
  pending_get_providers: HashMap<kad::QueryId, tokio_oneshot::Sender<HashSet<PeerId>>>,
  pending_request_file: HashMap<
    request_response::OutboundRequestId,
    tokio_oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>,
  >,
}

impl EventLoop {
  pub(crate) async fn run(mut self) {
    loop {
      tokio::select! {
        event = self.swarm.next() => {
          if let Some(event) = event {
            self.handle_event(event).await;
          }
        }
        command = self.command_receiver.recv() => {
          if let Some(command) = command {
            self.handle_command(command).await;
          }
        }
      }
    }
  }

  async fn handle_event(&mut self, event: SwarmEvent<ComposedBehaviourEvent>) {
    match event {
      SwarmEvent::Behaviour(ComposedBehaviourEvent::Kademlia(
        kad::Event::OutboundQueryProgressed {
          id,
          result: kad::QueryResult::StartProviding(_),
          ..
        },
      )) => {
        let sender: tokio_oneshot::Sender<()> = self
          .pending_start_providing
          .remove(&id)
          .expect("Completed query to be previously pending.");
        let _ = sender.send(());
      }
      SwarmEvent::Behaviour(ComposedBehaviourEvent::Kademlia(
        kad::Event::OutboundQueryProgressed {
          id,
          result:
            kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FoundProviders {
              providers, ..
            })),
          ..
        },
      )) => {
        if let Some(sender) = self.pending_get_providers.remove(&id) {
          let _ = sender.send(providers);
        }
      }
      SwarmEvent::Behaviour(ComposedBehaviourEvent::Kademlia(
        kad::Event::OutboundQueryProgressed {
          id,
          result:
            kad::QueryResult::GetProviders(Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord {
              ..
            })),
          ..
        },
      )) => {
        if let Some(sender) = self.pending_get_providers.remove(&id) {
          let _ = sender.send(HashSet::new());
        }
      }
      SwarmEvent::Behaviour(ComposedBehaviourEvent::RequestResponse(
        request_response::Event::Message {
          message: request_response::Message::Request {
            request, channel, ..
          },
          ..
        },
      )) => {
        self
          .event_sender
          .send(Event::InboundRequest {
            request: request.0,
            channel,
          })
          .expect("Event receiver not to be dropped.");
      }
      SwarmEvent::Behaviour(ComposedBehaviourEvent::RequestResponse(
        request_response::Event::Message {
          message:
            request_response::Message::Response {
              request_id,
              response,
            },
          ..
        },
      )) => {
        let _ = self
          .pending_request_file
          .remove(&request_id)
          .expect("Request to still be pending.")
          .send(Ok(response.data));
      }
      SwarmEvent::Behaviour(ComposedBehaviourEvent::RequestResponse(
        request_response::Event::OutboundFailure {
          request_id, error, ..
        },
      )) => {
        let _ = self
          .pending_request_file
          .remove(&request_id)
          .expect("Request to still be pending.")
          .send(Err(Box::new(SimpleError(format!("{:?}", error)))));
      }
      SwarmEvent::Behaviour(ComposedBehaviourEvent::RequestResponse(
        request_response::Event::ResponseSent { .. },
      )) => {}
      SwarmEvent::Behaviour(ComposedBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
        for (peer_id, multiaddr) in list {
          self
            .event_sender
            .send(Event::MdnsDiscovered {
              peer_id,
              addresses: vec![multiaddr.clone()],
            })
            .expect("Event receiver not to be dropped.");

          // Add discovered peer to Kademlia routing table
          self
            .swarm
            .behaviour_mut()
            .kademlia
            .add_address(&peer_id, multiaddr);
        }
      }
      SwarmEvent::Behaviour(ComposedBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
        for (peer_id, _multiaddr) in list {
          self
            .event_sender
            .send(Event::MdnsExpired { peer_id })
            .expect("Event receiver not to be dropped.");
        }
      }
      SwarmEvent::NewListenAddr { address, .. } => {
        println!("Local node is listening on {address}");
      }
      SwarmEvent::ConnectionEstablished {
        peer_id, endpoint, ..
      } => {
        if endpoint.is_dialer() {
          if let Some(sender) = self.pending_dial.remove(&peer_id) {
            let _ = sender.send(Ok(()));
          }
        }
      }
      SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
        if let Some(peer_id) = peer_id {
          if let Some(sender) = self.pending_dial.remove(&peer_id) {
            let _ = sender.send(Err(Box::new(SimpleError(format!("{:?}", error)))));
          }
        }
      }
      _ => {}
    }
  }

  async fn handle_command(&mut self, command: Command) {
    match command {
      Command::StartListening { addr, sender } => {
        let _ = match self.swarm.listen_on(addr) {
          Ok(_) => sender.send(Ok(())),
          Err(e) => sender.send(Err(Box::new(e))),
        };
      }
      Command::Dial {
        peer_id,
        peer_addr,
        sender,
      } => {
        if let Ok(()) = self
          .swarm
          .dial(peer_addr.with(libp2p::multiaddr::Protocol::P2p(peer_id)))
        {
          self.pending_dial.insert(peer_id, sender);
        } else {
          let _ = sender.send(Err(Box::new(SimpleError("Failed to dial".to_string()))));
        }
      }
      Command::StartProviding { file_name, sender } => {
        let query_id = self
          .swarm
          .behaviour_mut()
          .kademlia
          .start_providing(file_name.as_bytes().to_vec().into())
          .expect("No store error.");
        self.pending_start_providing.insert(query_id, sender);
      }
      Command::GetProviders { file_name, sender } => {
        let query_id = self
          .swarm
          .behaviour_mut()
          .kademlia
          .get_providers(file_name.as_bytes().to_vec().into());
        self.pending_get_providers.insert(query_id, sender);
      }
      Command::RequestFile {
        file_name,
        peer,
        sender,
      } => {
        let request_id = self
          .swarm
          .behaviour_mut()
          .request_response
          .send_request(&peer, FileRequest(file_name));
        self.pending_request_file.insert(request_id, sender);
      }
      Command::RespondFile { file, channel } => {
        self
          .swarm
          .behaviour_mut()
          .request_response
          .send_response(channel, FileResponse { data: file })
          .expect("Connection to peer to be still open.");
      }
    }
  }
}

#[derive(NetworkBehaviour)]
struct ComposedBehaviour {
  request_response: request_response::cbor::Behaviour<FileRequest, FileResponse>,
  kademlia: kad::Behaviour<kad::store::MemoryStore>,
  mdns: MdnsBehaviour,
}

#[derive(Debug)]
enum Command {
  StartListening {
    addr: Multiaddr,
    sender: tokio_oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
  },
  Dial {
    peer_id: PeerId,
    peer_addr: Multiaddr,
    sender: tokio_oneshot::Sender<Result<(), Box<dyn Error + Send>>>,
  },
  StartProviding {
    file_name: String,
    sender: tokio_oneshot::Sender<()>,
  },
  GetProviders {
    file_name: String,
    sender: tokio_oneshot::Sender<HashSet<PeerId>>,
  },
  RequestFile {
    file_name: String,
    peer: PeerId,
    sender: tokio_oneshot::Sender<Result<Vec<u8>, Box<dyn Error + Send>>>,
  },
  RespondFile {
    file: Vec<u8>,
    channel: ResponseChannel<FileResponse>,
  },
}

#[derive(Debug)]
pub(crate) enum Event {
  InboundRequest {
    request: String,
    channel: ResponseChannel<FileResponse>,
  },
  MdnsDiscovered {
    peer_id: PeerId,
    addresses: Vec<Multiaddr>,
  },
  MdnsExpired {
    peer_id: PeerId,
  },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FileRequest(String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileResponse {
  pub(crate) data: Vec<u8>,
}

pub(crate) async fn new(
  secret_key_seed: Option<u8>,
) -> Result<(Client, mpsc::UnboundedReceiver<Event>, EventLoop), Box<dyn Error>> {
  let id_keys = match secret_key_seed {
    Some(seed) => {
      let mut bytes = [0u8; 32];
      bytes[0] = seed;
      identity::Keypair::ed25519_from_bytes(bytes).expect("only errors on wrong length")
    }
    None => identity::Keypair::generate_ed25519(),
  };

  let peer_id = PeerId::from(id_keys.public());
  println!("Local peer id: {peer_id}");

  let mut swarm = SwarmBuilder::with_existing_identity(id_keys)
    .with_tokio()
    .with_tcp(
      tcp::Config::default(),
      noise::Config::new,
      yamux::Config::default,
    )
    .expect("Transport creation not to fail.")
    .with_behaviour(|key| {
      let store = kad::store::MemoryStore::new(key.public().to_peer_id());
      let kademlia = kad::Behaviour::new(key.public().to_peer_id(), store);
      let request_response = request_response::cbor::Behaviour::new(
        [(
          StreamProtocol::new("/file-exchange/1"),
          ProtocolSupport::Full,
        )],
        request_response::Config::default(),
      );
      let mdns = MdnsBehaviour::new(mdns::Config::default(), key.public().to_peer_id())
        .expect("Failed to create mDNS behaviour");

      ComposedBehaviour {
        request_response,
        kademlia,
        mdns,
      }
    })
    .expect("Behaviour creation not to fail.")
    .with_swarm_config(|c| c.with_idle_connection_timeout(std::time::Duration::from_secs(60)))
    .build();

  swarm
    .behaviour_mut()
    .kademlia
    .set_mode(Some(kad::Mode::Server));

  let (command_sender, command_receiver) = mpsc::unbounded_channel();
  let (event_sender, event_receiver) = mpsc::unbounded_channel();

  Ok((
    Client {
      sender: command_sender,
    },
    event_receiver,
    EventLoop {
      swarm,
      command_receiver,
      event_sender,
      pending_dial: Default::default(),
      pending_start_providing: Default::default(),
      pending_get_providers: Default::default(),
      pending_request_file: Default::default(),
    },
  ))
}

// Add missing import
use libp2p::StreamProtocol;

#[derive(Debug)]
pub struct SimpleError(String);

impl fmt::Display for SimpleError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl Error for SimpleError {}
