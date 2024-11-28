use libp2p::{
  Multiaddr, PeerId,
  gossipsub::{Behaviour as GossipsubBehavior, Event as GossipsubEvent},
  identify::{Behaviour as IdentifyBehavior, Event as IdentifyEvent},
  kad::{
    Behaviour as KademliaBehavior, Event as KademliaEvent, RoutingUpdate,
    store::MemoryStore as KademliaInMemory,
  },
  request_response::{
    Event as RequestResponseEvent, OutboundRequestId, ResponseChannel as RequestResponseChannel,
    cbor::Behaviour as RequestResponseBehavior,
  },
  swarm::NetworkBehaviour,
};

use crate::message::Message;

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "Event")]
pub(crate) struct Behavior {
  identify: IdentifyBehavior,
  kad: KademliaBehavior<KademliaInMemory>,
  rr: RequestResponseBehavior<Vec<u8>, Vec<u8>>,
  gossipsub: GossipsubBehavior,
}

impl Behavior {
  pub fn new(
    kad: KademliaBehavior<KademliaInMemory>,
    identify: IdentifyBehavior,
    rr: RequestResponseBehavior<Vec<u8>, Vec<u8>>,
    gossipsub: GossipsubBehavior,
  ) -> Self {
    Self {
      kad,
      identify,
      rr,
      gossipsub,
    }
  }

  pub fn register_addr_kad(&mut self, peer_id: &PeerId, addr: Multiaddr) -> RoutingUpdate {
    self.kad.add_address(peer_id, addr)
  }

  pub fn send_message(&mut self, peer_id: &PeerId, message: Message) -> OutboundRequestId {
    let binary_message = message.to_binary().expect("Failed to serialize message");
    self.rr.send_request(peer_id, binary_message)
  }

  pub fn send_response(
    &mut self,
    ch: RequestResponseChannel<Vec<u8>>,
    rs: Message,
  ) -> Result<(), Vec<u8>> {
    let binary_response = rs.to_binary().expect("Failed to serialize response");
    self.rr.send_response(ch, binary_response)
  }

  pub fn set_server_mode(&mut self) {
    self.kad.set_mode(Some(libp2p::kad::Mode::Server))
  }
}

#[derive(Debug)]
pub(crate) enum Event {
  Identify(IdentifyEvent),
  Kad(KademliaEvent),
  RequestResponse(RequestResponseEvent<Vec<u8>, Vec<u8>>),
  Gossipsub(GossipsubEvent),
}

impl From<IdentifyEvent> for Event {
  fn from(value: IdentifyEvent) -> Self {
    Self::Identify(value)
  }
}

impl From<KademliaEvent> for Event {
  fn from(value: KademliaEvent) -> Self {
    Self::Kad(value)
  }
}

impl From<RequestResponseEvent<Vec<u8>, Vec<u8>>> for Event {
  fn from(value: RequestResponseEvent<Vec<u8>, Vec<u8>>) -> Self {
    Self::RequestResponse(value)
  }
}

impl From<GossipsubEvent> for Event {
  fn from(value: GossipsubEvent) -> Self {
    Self::Gossipsub(value)
  }
}
