use libp2p::{
  Multiaddr, PeerId,
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

use crate::message::{GreeRequest, GreetResponse};

#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "Event")]
pub(crate) struct Behavior {
  identify: IdentifyBehavior,
  kad: KademliaBehavior<KademliaInMemory>,
  rr: RequestResponseBehavior<GreeRequest, GreetResponse>,
}

impl Behavior {
  pub fn new(
    kad: KademliaBehavior<KademliaInMemory>,
    identify: IdentifyBehavior,
    rr: RequestResponseBehavior<GreeRequest, GreetResponse>,
  ) -> Self {
    Self { kad, identify, rr }
  }

  pub fn register_addr_kad(&mut self, peer_id: &PeerId, addr: Multiaddr) -> RoutingUpdate {
    self.kad.add_address(peer_id, addr)
  }

  // pub fn register_addr_rr(&mut self, peer_id: &PeerId, addr: Multiaddr) -> bool {
  //     self.rr.add_address(peer_id, addr)
  // }

  pub fn send_message(&mut self, peer_id: &PeerId, message: GreeRequest) -> OutboundRequestId {
    self.rr.send_request(peer_id, message)
  }

  pub fn send_response(
    &mut self,
    ch: RequestResponseChannel<GreetResponse>,
    rs: GreetResponse,
  ) -> Result<(), GreetResponse> {
    self.rr.send_response(ch, rs)
  }

  pub fn set_server_mode(&mut self) {
    self.kad.set_mode(Some(libp2p::kad::Mode::Server))
  }
}

#[derive(Debug)]
pub(crate) enum Event {
  Identify(IdentifyEvent),
  Kad(KademliaEvent),
  RequestResponse(RequestResponseEvent<GreeRequest, GreetResponse>),
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

impl From<RequestResponseEvent<GreeRequest, GreetResponse>> for Event {
  fn from(value: RequestResponseEvent<GreeRequest, GreetResponse>) -> Self {
    Self::RequestResponse(value)
  }
}
