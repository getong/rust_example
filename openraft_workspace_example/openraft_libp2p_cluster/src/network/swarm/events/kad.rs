//! Kademlia event handling shared by the full-node loop (with a network
//! factory for peer registration) and the client loop (without).

use std::collections::HashSet;

use libp2p::{Multiaddr, PeerId, Swarm, kad};

use crate::network::{
  swarm::{Behaviour, NetErr, state::PendingKadTable},
  transport::Libp2pNetworkFactory,
};

pub(crate) fn handle_kad_event(
  swarm: &mut Swarm<Behaviour>,
  network: Option<&Libp2pNetworkFactory>,
  connected_peers: Option<&HashSet<PeerId>>,
  event: kad::Event,
  pending_kad: &mut PendingKadTable,
) {
  match event {
    kad::Event::RoutingUpdated {
      peer, addresses, ..
    } => {
      if peer == *swarm.local_peer_id() {
        return;
      }
      if let Some(network) = network {
        let addrs: Vec<Multiaddr> = addresses.iter().cloned().collect();
        let network = network.clone();
        tokio::spawn(async move {
          for addr in addrs {
            let _ = network.register_discovered_peer(peer, addr).await;
          }
        });
      }
      if connected_peers.is_none() {
        tracing::debug!(peer = %peer, "kad routing updated (client)");
        return;
      }
      tracing::debug!(
        peer = %peer,
        addresses = ?addresses,
        "kad routing updated"
      );
      if connected_peers.is_some_and(|peers| peers.contains(&peer)) {
        return;
      }
    }
    kad::Event::OutboundQueryProgressed { id, result, .. } => match result {
      kad::QueryResult::GetClosestPeers(result) => match result {
        Ok(ok) => {
          if ok.peers.is_empty() {
            tracing::debug!("kad get_closest_peers complete: no peers");
          } else {
            tracing::debug!(peers = ?ok.peers, "kad get_closest_peers complete");
          }
        }
        Err(err) => {
          tracing::debug!(error = ?err, "kad get_closest_peers failed");
        }
      },
      kad::QueryResult::StartProviding(result) => {
        if let Some(resp) = pending_kad.start_providing.remove(&id) {
          match result {
            Ok(_) => {
              let _ = resp.send(Ok(()));
            }
            Err(e) => {
              let _ = resp.send(Err(NetErr(format!(
                "kademlia start_providing failed: {:?}",
                e
              ))));
            }
          }
        }
      }
      kad::QueryResult::GetProviders(result) => match result {
        Ok(kad::GetProvidersOk::FoundProviders { key: _, providers }) => {
          if let Some(state) = pending_kad.get_providers.get_mut(&id) {
            state.providers.extend(providers);
          }
        }
        Ok(kad::GetProvidersOk::FinishedWithNoAdditionalRecord { .. }) => {
          if let Some(state) = pending_kad.get_providers.remove(&id) {
            let _ = state.resp.send(Ok(state.providers));
          }
        }
        Err(e) => {
          if let Some(state) = pending_kad.get_providers.remove(&id) {
            let _ = state.resp.send(Err(NetErr(format!(
              "kademlia get_providers failed: {:?}",
              e
            ))));
          }
        }
      },
      _ => {}
    },
    other => {
      tracing::debug!("kad event: {:?}", other);
    }
  }
}
