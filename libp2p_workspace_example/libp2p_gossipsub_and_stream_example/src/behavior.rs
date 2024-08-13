use libp2p::{
  gossipsub::Behaviour as GossipsubBehavior,
  identify::Behaviour as IdentifyBehavior,
  kad::{store::MemoryStore as KademliaInMemory, Behaviour as KademliaBehavior, RoutingUpdate},
  mdns::tokio::Behaviour as MdnsBehavior,
  request_response::{
    cbor::Behaviour as RequestResponseBehavior, OutboundRequestId,
    ResponseChannel as RequestResponseChannel,
  },
  swarm::NetworkBehaviour,
  Multiaddr, PeerId,
};

use libp2p_stream::Behaviour as StreamBehavior;

use crate::message::{GreeRequest, GreetResponse};

#[derive(NetworkBehaviour)]
pub(crate) struct Behavior {
  pub identify: IdentifyBehavior,
  pub kad: KademliaBehavior<KademliaInMemory>,
  pub rr: RequestResponseBehavior<GreeRequest, GreetResponse>,
  pub gossipsub: GossipsubBehavior,
  pub mdns: MdnsBehavior,
  pub stream: StreamBehavior,
}

impl Behavior {
  pub fn new(
    kad: KademliaBehavior<KademliaInMemory>,
    identify: IdentifyBehavior,
    rr: RequestResponseBehavior<GreeRequest, GreetResponse>,
    gossipsub: GossipsubBehavior,
    mdns: MdnsBehavior,
    stream: StreamBehavior,
  ) -> Self {
    Self {
      kad,
      identify,
      rr,
      gossipsub,
      mdns,
      stream,
    }
  }

  pub fn register_addr_kad(&mut self, peer_id: &PeerId, addr: Multiaddr) -> RoutingUpdate {
    self.kad.add_address(peer_id, addr)
  }

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
