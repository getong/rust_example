//! libp2p swarm ownership and event loops.
//!
//! Module layout (see fourth_ARCHITECTURE_REVIEW.md §4.1):
//!   - `client`    — typed client handles over the command channel
//!   - `commands`  — the `Command` enum and its handlers
//!   - `state`     — `SwarmState` and the pending tables
//!   - `dial`      — dialing / kademlia address-book helpers
//!   - `reconnect` — periodic reconnect logic
//!   - `events/*`  — per-behaviour swarm event handlers

pub(crate) mod client;
pub(crate) mod commands;
pub(crate) mod dial;
pub(crate) mod events;
pub(crate) mod reconnect;
pub(crate) mod state;

use std::{sync::Arc, time::Duration};

pub use client::{
  CommandSenders, KvClient, Libp2pClient, SqliteSyncClient, TaskRpcClient, WasmSyncClient,
};
pub use commands::{Command, GossipTopicReport, Libp2pSwarmReport};
use futures::StreamExt;
use libp2p::{
  gossipsub,
  kad::{self, store::MemoryStore},
  mdns, ping, request_response,
  swarm::{NetworkBehaviour, Swarm, SwarmEvent},
};
use tokio::sync::mpsc;

use self::{
  commands::{handle_command, handle_command_batch},
  dial::{add_kad_peer_address, leave_openraft_kad, strip_p2p},
  events::{
    connection::{handle_connection_established, handle_outgoing_connection_error},
    gossipsub::{NODE_ANNOUNCE_QUEUE, run_node_announce_processor},
    kad::handle_kad_event,
    ping::handle_ping_event,
    rpc::kv_error_response,
  },
  reconnect::handle_reconnect_tick,
  state::{SwarmState, record_swarm_gauges},
};
use crate::{
  network::{
    dispatcher::SwarmRequestDispatcher,
    proto_codec::UnifiedCodec,
    rpc::{RaftRpcResponse, UnifiedRpcRequest, UnifiedRpcResponse},
    transport::Libp2pNetworkFactory,
  },
  signal::ShutdownRx,
};

pub const GOSSIP_TOPIC: &str = "openraft/cluster/1";
pub const NODE_ANNOUNCE_TOPIC: &str = "openraft/node-announce/1";
/// Per-node wasm module inventory announcements (see [`crate::wasm_sync`]).
pub const WASM_MODULES_TOPIC: &str = "openraft/wasm-modules/1";
/// Gossipsub `mesh_n_low`: the degree below which the mesh is considered
/// under-connected. Shared with the mesh-health signal so "healthy" means
/// "at or above the maintenance low-water mark".
pub const GOSSIPSUB_MESH_N_LOW: usize = 4;
pub const OPENRAFT_CLUSTER_PROVIDER_KEY: &str = "openraft_cluster";
/// Single request-response protocol carrying every RPC kind (raft / kv /
/// sqlite-sync / task) as a tagged envelope. One protocol means one
/// negotiation per connection and one pending table in the swarm loop,
/// replacing the former four per-kind protocols.
pub const UNIFIED_RPC_PROTOCOL: &str = "/openraft/rpc/2";
pub const KAD_PROTOCOL: &str = "/openraft/kad/1";
/// Inbound + outbound stream cap for the unified request-response
/// behaviour. The libp2p default (100) is below the node's own concurrency
/// budget — the outbound guard admits up to 256 in-flight RPCs
/// (`RPC_GLOBAL_MAX_CONCURRENT`) and the inbound dispatcher another 256 —
/// so under raft replication fan-out plus a KV burst the stream cap, not
/// the RPC guards, would become the throughput ceiling. 512 covers both
/// directions at their configured maximums; the muxer layers underneath
/// (yamux `max_num_streams` defaults to 512 per connection, QUIC streams
/// are cheaper still) already allow this much parallelism per connection.
pub const RPC_MAX_CONCURRENT_STREAMS: usize = 512;
/// libp2p-layer timeout for one unified-RPC request/response exchange.
/// This must exceed the LONGEST application-level RPC timeout, because the
/// request-response behaviour kills the substream when it fires regardless
/// of what the caller is still willing to wait for. Snapshot transfers are
/// the slowest RPC kind (tens of seconds for multi-megabyte state machines
/// on a congested link), so this is sized for them; per-kind caller-side
/// timeouts (see `Libp2pClient::request`) keep fast RPCs failing fast.
pub const RPC_PROTOCOL_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Max commands drained from the command channel per loop wakeup. Batching
/// amortizes the `tokio::select!` round-trip: under RPC bursts (raft
/// replication fan-out, response sends funneled back from spawned handlers)
/// many commands are already queued, and handling them one-per-wakeup makes
/// the single swarm loop the throughput ceiling.
///
/// Why 64: a quarter of the command channel capacity (256), so one drain
/// clears a burst that filled a quarter of the queue, while the batch stays
/// small enough that the other select! arms (shutdown, swarm events) are
/// polled again within microseconds — command handlers only mutate behaviour
/// state and complete oneshots, they never await.
const SWARM_CMD_BATCH: usize = 64;
/// Max additional already-ready swarm events drained after the one that woke
/// the loop, before commands get a turn again. Bounded so a firehose of
/// events cannot starve command processing.
///
/// Why 32 (half of `SWARM_CMD_BATCH`): event handling is heavier per item
/// than command handling — inbound requests spawn dispatch tasks and
/// connection events take address-book locks — and events arrive one poll at
/// a time anyway, so a smaller opportunistic drain keeps the raft command
/// channel's worst-case wait short.
const SWARM_EVENT_BATCH: usize = 32;

/// Gossipsub configuration shared by the full node and the client binaries.
///
/// The knobs that matter at scale are set explicitly instead of relying on
/// library defaults:
///   - `flood_publish(false)`: publish into the mesh (bounded degree) instead of to every connected
///     subscriber, so a node with many live connections (e.g. the raft leader) does not pay
///     O(connections) per publish;
///   - explicit mesh degree bounds keep the per-node gossip fan-out constant no matter how many
///     peers are known;
///   - `max_transmit_size` fits an openraft snapshot partial message (4 x 8 KiB parts +
///     metadata/bitmap) with generous headroom.
pub fn build_gossipsub_config() -> Result<gossipsub::Config, gossipsub::ConfigBuilderError> {
  gossipsub::ConfigBuilder::default()
    .heartbeat_interval(Duration::from_secs(1))
    .mesh_n(6)
    .mesh_n_low(GOSSIPSUB_MESH_N_LOW)
    .mesh_n_high(12)
    .flood_publish(false)
    .max_transmit_size(128 * 1024)
    .build()
}

// The swarm layer's error type is the unified [`ClusterError`]
// (`Network` variant); re-exported here because this module is where most
// of its producers and consumers live.
pub use crate::error::ClusterError;

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "BehaviourEvent")]
pub struct Behaviour {
  pub rpc: request_response::Behaviour<UnifiedCodec>,
  pub gossipsub: gossipsub::Behaviour,
  pub ping: ping::Behaviour,
  pub mdns: mdns::tokio::Behaviour,
  pub kad: kad::Behaviour<MemoryStore>,
}

#[derive(Debug)]
pub enum BehaviourEvent {
  Rpc(request_response::Event<UnifiedRpcRequest, UnifiedRpcResponse>),
  Gossipsub(gossipsub::Event),
  Ping(ping::Event),
  Mdns(mdns::Event),
  Kad(kad::Event),
}

impl From<request_response::Event<UnifiedRpcRequest, UnifiedRpcResponse>> for BehaviourEvent {
  fn from(value: request_response::Event<UnifiedRpcRequest, UnifiedRpcResponse>) -> Self {
    Self::Rpc(value)
  }
}

impl From<gossipsub::Event> for BehaviourEvent {
  fn from(value: gossipsub::Event) -> Self {
    Self::Gossipsub(value)
  }
}

impl From<ping::Event> for BehaviourEvent {
  fn from(value: ping::Event) -> Self {
    Self::Ping(value)
  }
}

impl From<mdns::Event> for BehaviourEvent {
  fn from(value: mdns::Event) -> Self {
    Self::Mdns(value)
  }
}

impl From<kad::Event> for BehaviourEvent {
  fn from(value: kad::Event) -> Self {
    Self::Kad(value)
  }
}

/// Everything the full-node swarm loop needs, bundled so `run_swarm` takes
/// one argument instead of eight and call sites cannot silently swap two
/// same-typed parameters (the two command receivers, most dangerously).
pub struct SwarmContext {
  pub swarm: Swarm<Behaviour>,
  /// Raft traffic (vote / append-entries / snapshot): drained first.
  pub cmd_rx_high: mpsc::Receiver<Command>,
  /// KV / sqlite-sync / task RPC and connection management.
  pub cmd_rx_low: mpsc::Receiver<Command>,
  /// Senders back into the loop, used by spawned request handlers to funnel
  /// their responses through the swarm.
  pub cmd_tx: CommandSenders,
  pub network: Libp2pNetworkFactory,
  pub dispatcher: Arc<dyn SwarmRequestDispatcher>,
  pub registry: crate::GroupRegistry,
  pub shutdown_rx: ShutdownRx,
}

/// Full-node swarm loop.
///
/// The loop owns the `Swarm` exclusively: every other component interacts
/// with it through the `Command` mpsc channels, never through a shared lock.
/// Long-running work (request dispatch, snapshot building/installing) is
/// spawned onto separate tasks so the loop stays a thin, non-blocking
/// multiplexer over swarm events and commands.
pub async fn run_swarm(ctx: SwarmContext) {
  let SwarmContext {
    mut swarm,
    mut cmd_rx_high,
    mut cmd_rx_low,
    cmd_tx,
    network,
    dispatcher,
    registry,
    mut shutdown_rx,
  } = ctx;
  let mut state = SwarmState::default();
  // 12s: several raft election timeouts, so a pinned peer that dropped is
  // redialed well before the membership guard's replace timeout (minutes),
  // yet slow enough that N nodes redialing a dead peer stay negligible.
  let mut reconnect_tick = tokio::time::interval(Duration::from_secs(12));
  reconnect_tick.tick().await;

  // Node announcements are decoded and registered off-loop: the volume is
  // O(cluster size) and registration takes address-book locks. The processor
  // task ends by itself when the swarm loop drops `announce_tx`.
  let (announce_tx, announce_rx) = mpsc::channel::<Vec<u8>>(NODE_ANNOUNCE_QUEUE);
  tokio::spawn(run_node_announce_processor(network.clone(), announce_rx));

  let mut cmd_batch_high: Vec<Command> = Vec::with_capacity(SWARM_CMD_BATCH);
  let mut cmd_batch_low: Vec<Command> = Vec::with_capacity(SWARM_CMD_BATCH);
  let mut event_batch: Vec<SwarmEvent<BehaviourEvent>> = Vec::with_capacity(SWARM_EVENT_BATCH);
  loop {
    tokio::select! {
      // Fixed polling order: shutdown, then raft commands, then swarm
      // events, then low-priority commands. Raft commands are rate-bounded
      // by raft itself (heartbeat cadence, one election at a time), so
      // preferring them cannot starve the rest.
      biased;
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm");
        leave_openraft_kad(&mut swarm);
        state.fail_all_pending("swarm shutting down");
        break;
      }
      n = cmd_rx_high.recv_many(&mut cmd_batch_high, SWARM_CMD_BATCH) => {
        if n == 0 {
          tracing::warn!("swarm command channel closed; stopping swarm");
          leave_openraft_kad(&mut swarm);
          state.fail_all_pending("swarm command channel closed");
          return;
        }
        handle_command_batch(&mut swarm, &mut cmd_batch_high, &mut state);
      }

      ev = swarm.next() => {
        let Some(ev) = ev else {
          tracing::warn!("swarm stream ended; stopping swarm");
          state.fail_all_pending("swarm stream ended");
          break;
        };
        // Opportunistically drain events that are already ready, so a burst
        // of RPC traffic is handled in one wakeup instead of paying the
        // select! round-trip per event. A `Ready(None)` mid-drain is left
        // for the next loop iteration to observe and shut down on.
        event_batch.push(ev);
        while event_batch.len() < SWARM_EVENT_BATCH {
          match futures::poll!(swarm.next()) {
            std::task::Poll::Ready(Some(ev)) => event_batch.push(ev),
            _ => break,
          }
        }
        for ev in event_batch.drain(..) {
          events::handle_swarm_event(
            &mut swarm,
            ev,
            &network,
            dispatcher.clone(),
            &registry,
            &cmd_tx,
            &announce_tx,
            &mut state,
          )
          .await;
        }
      }

      n = cmd_rx_low.recv_many(&mut cmd_batch_low, SWARM_CMD_BATCH) => {
        if n == 0 {
          tracing::warn!("swarm command channel closed; stopping swarm");
          leave_openraft_kad(&mut swarm);
          state.fail_all_pending("swarm command channel closed");
          return;
        }
        handle_command_batch(&mut swarm, &mut cmd_batch_low, &mut state);
      }

      _ = reconnect_tick.tick() => {
        record_swarm_gauges(&state);
        handle_reconnect_tick(
          &mut swarm,
          &network,
          &state.connected_peers,
          &mut state.reconnect_backoff_until,
        ).await;
      }
    }
  }
}

/// Client-only swarm loop.
///
/// It supports outbound requests/responses but does not require a `Raft` handle.
/// If it receives an inbound request, it responds with `RaftRpcResponse::Error`.
pub async fn run_swarm_client(swarm: Swarm<Behaviour>, cmd_rx: mpsc::Receiver<Command>) {
  let (_shutdown_tx, shutdown_rx) = crate::signal::channel();
  run_swarm_client_with_shutdown(swarm, cmd_rx, shutdown_rx).await;
}

pub async fn run_swarm_client_with_shutdown(
  mut swarm: Swarm<Behaviour>,
  mut cmd_rx: mpsc::Receiver<Command>,
  mut shutdown_rx: ShutdownRx,
) {
  let mut state = SwarmState::default();

  loop {
    tokio::select! {
      _ = shutdown_rx.changed() => {
        tracing::info!("shutdown signal received, stopping swarm client");
        leave_openraft_kad(&mut swarm);
        state.fail_all_pending("swarm client shutting down");
        break;
      }
      cmd = cmd_rx.recv() => {
        let Some(cmd) = cmd else {
          tracing::warn!("swarm client command channel closed; stopping swarm client");
          leave_openraft_kad(&mut swarm);
          state.fail_all_pending("swarm client command channel closed");
          return;
        };
        handle_command(&mut swarm, cmd, &mut state);
      }

      ev = swarm.next() => {
        let Some(ev) = ev else {
          tracing::warn!("swarm client stream ended; stopping swarm client");
          state.fail_all_pending("swarm client stream ended");
          break;
        };
        match ev {
          SwarmEvent::Behaviour(BehaviourEvent::Rpc(event)) => match event {
            request_response::Event::Message { message, .. } => match message {
              request_response::Message::Request {
                request, channel, ..
              } => match request {
                // Client binaries serve no raft groups; answer with a typed
                // error instead of hanging the requester.
                UnifiedRpcRequest::Raft(_) => {
                  let _ = swarm.behaviour_mut().rpc.send_response(
                    channel,
                    UnifiedRpcResponse::Raft(RaftRpcResponse::Error("client-only".to_string())),
                  );
                }
                UnifiedRpcRequest::Kv(_) => {
                  let _ = swarm.behaviour_mut().rpc.send_response(
                    channel,
                    UnifiedRpcResponse::Kv(kv_error_response("client-only")),
                  );
                }
                UnifiedRpcRequest::SqliteSync(request) => {
                  // Client binaries serve no raft groups: an empty registry
                  // makes every group lookup answer "not enabled here".
                  let response = crate::sqlite_cache::process_sqlite_sync_rpc_request(
                    request,
                    crate::GroupRegistry::default(),
                  )
                  .await;
                  let _ = swarm
                    .behaviour_mut()
                    .rpc
                    .send_response(channel, UnifiedRpcResponse::SqliteSync(response));
                }
                // No task worker runs in client binaries; dropping the
                // channel fails the request on the requester side.
                UnifiedRpcRequest::Task(_) => {}
                UnifiedRpcRequest::WasmSync(_) => {
                  let _ = swarm.behaviour_mut().rpc.send_response(
                    channel,
                    UnifiedRpcResponse::WasmSync(crate::wasm_sync::WasmSyncResponse::Error(
                      "client-only".to_string(),
                    )),
                  );
                }
              },
              request_response::Message::Response { request_id, response } => {
                state.pending_rpc.complete(&request_id, Ok(response));
              }
            },

            request_response::Event::OutboundFailure { request_id, error, .. } => {
              state.pending_rpc.complete(
                &request_id,
                Err(ClusterError::Network(format!("outbound failure: {error}"))),
              );
            }

            request_response::Event::InboundFailure { .. } => {}
            request_response::Event::ResponseSent { .. } => {}
          },

          SwarmEvent::Behaviour(BehaviourEvent::Mdns(event)) => match event {
            mdns::Event::Discovered(list) => {
              // New routing-table entries trigger kad's automatic bootstrap.
              for (peer, addr) in list {
                add_kad_peer_address(&mut swarm, peer, addr);
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
          },

          SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(event)) => {
            tracing::debug!("gossipsub event: {:?}", event);
          }

          SwarmEvent::Behaviour(BehaviourEvent::Ping(event)) => {
            handle_ping_event(&mut swarm, &mut state.ping_failures, event);
          }

          SwarmEvent::Behaviour(BehaviourEvent::Kad(event)) => {
            handle_kad_event(&mut swarm, None, None, event, &mut state.pending_kad);
          }

          SwarmEvent::ConnectionEstablished { peer_id, .. } => {
            state.outgoing_failure_log_backoff_until.remove(&peer_id);
            handle_connection_established(
              &mut swarm,
              &mut state.pending_connect,
              &mut state.connected_peers,
              &mut state.dial_backoff_until,
              peer_id,
            );
          }

          SwarmEvent::ConnectionClosed { peer_id, connection_id, num_established, .. } => {
            state.ping_failures.remove(&connection_id);
            if num_established == 0 {
              state.connected_peers.remove(&peer_id);
              swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
            }
          }

          SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
            handle_outgoing_connection_error(
              &mut swarm,
              &mut state.pending_connect,
              &mut state.dial_backoff_until,
              &mut state.outgoing_failure_log_backoff_until,
              peer_id,
              error,
            );
          }

          SwarmEvent::NewListenAddr { address, .. } => {
            tracing::info!("listening on {address}");
          }

          _ => {}
        }
      }
    }
  }
}
