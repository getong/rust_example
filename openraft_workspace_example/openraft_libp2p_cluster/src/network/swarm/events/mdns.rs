//! mDNS discovery event handling for the full-node loop.

use libp2p::{Swarm, mdns};

use crate::network::{
  swarm::{
    Behaviour,
    dial::{add_kad_peer_address, strip_p2p},
  },
  transport::Libp2pNetworkFactory,
};

pub(crate) async fn handle_mdns_event(
  swarm: &mut Swarm<Behaviour>,
  network: &Libp2pNetworkFactory,
  event: mdns::Event,
) {
  match event {
    mdns::Event::Discovered(list) => {
      for (peer, addr) in list {
        if crate::network::transport::is_undialable_discovered_addr(&addr) {
          continue;
        }
        let mut use_discovered_addr = network.update_peer_addr_from_mdns(peer, addr.clone()).await;
        if network.register_discovered_peer(peer, addr.clone()).await {
          use_discovered_addr = true;
        }
        if use_discovered_addr {
          // New routing-table entries trigger kad's automatic bootstrap.
          add_kad_peer_address(swarm, peer, addr);
        }
        swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer);
      }
    }
    mdns::Event::Expired(list) => {
      for (peer, addr) in list {
        let addr = strip_p2p(addr);
        swarm.behaviour_mut().kad.remove_address(&peer, &addr);
        swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer);
      }
    }
  }
}
