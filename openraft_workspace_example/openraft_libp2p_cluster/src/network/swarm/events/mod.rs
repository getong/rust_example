//! Per-behaviour swarm event handlers. `handle_swarm_event` is the single
//! entry point used by the full-node loop; each behaviour's logic lives in
//! its own submodule.

pub(crate) mod connection;
pub(crate) mod gossipsub;
pub(crate) mod kad;
pub(crate) mod mdns;
pub(crate) mod ping;
pub(crate) mod rpc;

use std::sync::Arc;

use libp2p::{Swarm, swarm::SwarmEvent};
use tokio::sync::mpsc;

use super::{Behaviour, BehaviourEvent, client::CommandSenders, state::SwarmState};
use crate::network::{dispatcher::SwarmRequestDispatcher, transport::Libp2pNetworkFactory};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_swarm_event(
  swarm: &mut Swarm<Behaviour>,
  event: SwarmEvent<BehaviourEvent>,
  network: &Libp2pNetworkFactory,
  dispatcher: Arc<dyn SwarmRequestDispatcher>,
  registry: &crate::GroupRegistry,
  cmd_tx: &CommandSenders,
  announce_tx: &mpsc::Sender<Vec<u8>>,
  state: &mut SwarmState,
) {
  match event {
    SwarmEvent::Behaviour(BehaviourEvent::Rpc(event)) => {
      // Any RPC message (inbound request or response) counts as activity:
      // the connection janitor spares recently active peers, so in-flight
      // exchanges are not cut mid-conversation.
      if let libp2p::request_response::Event::Message { peer, .. } = &event {
        state
          .last_peer_activity
          .insert(*peer, tokio::time::Instant::now());
      }
      rpc::handle_rpc_event(
        swarm,
        dispatcher.clone(),
        cmd_tx,
        state.inbound_dispatch_limit.clone(),
        &mut state.pending_rpc,
        event,
      );
    }
    SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => {
      mdns::handle_mdns_event(swarm, network, event).await;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(event)) => {
      gossipsub::handle_gossipsub_event(
        swarm,
        announce_tx,
        registry,
        &mut state.openraft_sync,
        event,
      )
      .await;
    }
    SwarmEvent::Behaviour(BehaviourEvent::Ping(event)) => {
      ping::handle_ping_event(swarm, &mut state.ping_failures, event);
    }
    SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
      kad::handle_kad_event(
        swarm,
        Some(network),
        Some(&state.connected_peers),
        event,
        &mut state.pending_kad,
      );
    }
    SwarmEvent::ConnectionEstablished { peer_id, .. } => {
      connection::handle_connection_established(
        &mut state.pending_connect,
        &mut state.connected_peers,
        &mut state.dial_backoff_until,
        peer_id,
      );
      state.reconnect_backoff_until.remove(&peer_id);
      state.outgoing_failure_log_backoff_until.remove(&peer_id);
      // A fresh connection counts as activity so the janitor gives the
      // dialer's first RPC a grace window before considering it surplus.
      state
        .last_peer_activity
        .insert(peer_id, tokio::time::Instant::now());
      network.set_peer_connected(peer_id).await;
    }
    SwarmEvent::ConnectionClosed {
      peer_id,
      connection_id,
      num_established,
      cause,
      ..
    } => {
      state.ping_failures.remove(&connection_id);
      connection::handle_connection_closed(
        &mut state.connected_peers,
        peer_id,
        num_established,
        cause,
      );
      if num_established == 0 {
        state.last_peer_activity.remove(&peer_id);
        network.set_peer_disconnected(peer_id).await;
      }
    }
    SwarmEvent::NewListenAddr { address, .. } => {
      tracing::info!("listening on {address}");
    }
    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
      connection::handle_outgoing_connection_error(
        swarm,
        &mut state.pending_connect,
        &mut state.dial_backoff_until,
        &mut state.outgoing_failure_log_backoff_until,
        peer_id,
        error,
      );
    }
    _ => {}
  }
}
