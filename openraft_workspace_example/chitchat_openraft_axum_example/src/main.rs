use std::{
  collections::HashMap,
  sync::Arc,
  time::{Duration, SystemTime},
};

use axum::{
  Router,
  extract::{Query, State},
  response::Json,
  routing::{delete, get, post},
};
use chitchat::{
  ChitchatConfig, ChitchatId, FailureDetectorConfig, spawn_chitchat, transport::UdpTransport,
};
use chitchat_openraft_axum_example::{
  ApiResponse, ConsistencyLevel, ConsistencyRouter, ConsistencyStats, HybridApiResponse,
  SetKeyValueResponse,
};
use serde::Deserialize;
use tokio::sync::Mutex;

/// Application state with proper OpenRaft integration preparation
#[derive(Clone)]
struct HybridAppState {
  chitchat: Arc<chitchat::ChitchatHandle>,
  consistency_router: Arc<ConsistencyRouter>,
  stats: Arc<Mutex<ConsistencyStats>>,
  // Raft storage - prepared for real OpenRaft integration
  raft_data: Arc<Mutex<HashMap<String, String>>>,
  // TODO: Add actual OpenRaft instance when API compatibility is resolved
  // raft: Arc<openraft::Raft<TypeConfig>>,
}

#[derive(Debug, Deserialize)]
struct SetKeyValueParams {
  key: String,
  value: String,
  consistency: Option<String>, // "eventual" | "strong" | "hybrid"
}

#[derive(Debug, Deserialize)]
struct DeleteKeyParams {
  key: String,
  consistency: Option<String>,
}

/// Simple index endpoint
async fn index() -> &'static str {
  "Chitchat-OpenRaft Hybrid Server (Simplified)"
}

/// Get the current hybrid cluster state
async fn get_hybrid_state(State(state): State<HybridAppState>) -> Json<HybridApiResponse> {
  let chitchat = state.chitchat.chitchat();
  let chitchat_guard = chitchat.lock().await;
  let stats = state.stats.lock().await.clone();
  let raft_data = state.raft_data.lock().await.clone();

  let response = HybridApiResponse {
    cluster_id: chitchat_guard.cluster_id().to_string(),
    chitchat_state: chitchat_guard.state_snapshot(),
    raft_state: raft_data,
    live_nodes: chitchat_guard.live_nodes().cloned().collect::<Vec<_>>(),
    dead_nodes: chitchat_guard.dead_nodes().cloned().collect::<Vec<_>>(),
    consistency_stats: stats,
  };
  Json(response)
}

/// Legacy endpoint for backward compatibility
async fn get_state(State(state): State<HybridAppState>) -> Json<ApiResponse> {
  let chitchat = state.chitchat.chitchat();
  let chitchat_guard = chitchat.lock().await;
  let response = ApiResponse {
    cluster_id: chitchat_guard.cluster_id().to_string(),
    cluster_state: chitchat_guard.state_snapshot(),
    live_nodes: chitchat_guard.live_nodes().cloned().collect::<Vec<_>>(),
    dead_nodes: chitchat_guard.dead_nodes().cloned().collect::<Vec<_>>(),
  };
  Json(response)
}

/// Set a key-value pair with configurable consistency
async fn set_kv_v2(
  State(state): State<HybridAppState>,
  Query(params): Query<SetKeyValueParams>,
) -> Json<SetKeyValueResponse> {
  let consistency_level = params
    .consistency
    .as_deref()
    .and_then(|s| s.parse::<ConsistencyLevel>().ok())
    .unwrap_or_else(|| state.consistency_router.route_for_key(&params.key));

  let mut stats = state.stats.lock().await;

  match consistency_level {
    ConsistencyLevel::Strong => {
      // Store in raft placeholder for now
      tracing::info!("Using Raft simulation for key: {}", params.key);
      let mut raft_data = state.raft_data.lock().await;
      raft_data.insert(params.key.clone(), params.value.clone());
      stats.increment_raft();
    }
    ConsistencyLevel::Eventual => {
      // Use Chitchat for eventual consistency
      tracing::info!("Using Chitchat for key: {}", params.key);
      let chitchat = state.chitchat.chitchat();
      let mut chitchat_guard = chitchat.lock().await;
      let cc_state = chitchat_guard.self_node_state();
      cc_state.set(&params.key, &params.value);
      stats.increment_chitchat();
    }
    ConsistencyLevel::Hybrid => {
      // Write to both systems
      tracing::info!("Using Hybrid approach for key: {}", params.key);

      // Write to Chitchat
      let chitchat = state.chitchat.chitchat();
      let mut chitchat_guard = chitchat.lock().await;
      let cc_state = chitchat_guard.self_node_state();
      cc_state.set(&params.key, &params.value);
      drop(chitchat_guard);

      // Also write to raft placeholder
      let mut raft_data = state.raft_data.lock().await;
      raft_data.insert(params.key.clone(), params.value.clone());

      stats.increment_hybrid();
    }
  }

  Json(SetKeyValueResponse { status: true })
}

/// Legacy set_kv endpoint for backward compatibility
async fn set_kv(
  State(state): State<HybridAppState>,
  Query(params): Query<SetKeyValueParams>,
) -> Json<SetKeyValueResponse> {
  let new_params = SetKeyValueParams {
    key: params.key,
    value: params.value,
    consistency: None, // Will use router default
  };

  set_kv_v2(State(state), Query(new_params)).await
}

/// Mark a key for deletion with configurable consistency
async fn mark_for_deletion(
  State(state): State<HybridAppState>,
  Query(params): Query<DeleteKeyParams>,
) -> Json<SetKeyValueResponse> {
  let consistency_level = params
    .consistency
    .as_deref()
    .and_then(|s| s.parse::<ConsistencyLevel>().ok())
    .unwrap_or_else(|| state.consistency_router.route_for_key(&params.key));

  match consistency_level {
    ConsistencyLevel::Strong => {
      tracing::info!("Using Raft simulation to delete key: {}", params.key);
      let mut raft_data = state.raft_data.lock().await;
      raft_data.remove(&params.key);
    }
    ConsistencyLevel::Eventual => {
      tracing::info!("Using Chitchat to delete key: {}", params.key);
      let chitchat = state.chitchat.chitchat();
      let mut chitchat_guard = chitchat.lock().await;
      let cc_state = chitchat_guard.self_node_state();
      cc_state.delete(&params.key);
    }
    ConsistencyLevel::Hybrid => {
      tracing::info!("Using Hybrid approach to delete key: {}", params.key);
      let chitchat = state.chitchat.chitchat();
      let mut chitchat_guard = chitchat.lock().await;
      let cc_state = chitchat_guard.self_node_state();
      cc_state.delete(&params.key);
      drop(chitchat_guard);

      let mut raft_data = state.raft_data.lock().await;
      raft_data.remove(&params.key);
    }
  }

  Json(SetKeyValueResponse { status: true })
}

/// TODO: OpenRaft initialization function based on working example pattern
/// This shows how to properly start OpenRaft when API compatibility is resolved
///
/// Based on: https://github.com/databendlabs/openraft/tree/main/examples/raft-kv-memstore-network-v2/src
///
/// Key steps for proper OpenRaft initialization:
/// 1. Create configuration with timeouts and settings
/// 2. Create LogStore and StateMachineStore instances
/// 3. Create Raft instance with openraft::Raft::new()
/// 4. Initialize cluster membership with raft.initialize()
/// 5. Create App that handles raft requests in a loop
/// 6. Run the App loop in a background task
///
/// ```rust
/// async fn initialize_openraft(node_id: u64) -> anyhow::Result<openraft::Raft<TypeConfig>> {
///   use openraft::Config;
///
///   // Create a configuration for the raft instance
///   let config = Config {
///     heartbeat_interval: 500,
///     election_timeout_min: 1500,
///     election_timeout_max: 3000,
///     max_in_snapshot_log_to_keep: 0,
///     ..Default::default()
///   };
///   let config = Arc::new(config.validate().unwrap());
///
///   // Create stores
///   let log_store = LogStore::default();
///   let state_machine = Arc::new(StateMachineStore::default());
///
///   // Create network router
///   let router = Router::new();
///
///   // Create the Raft instance
///   let raft = openraft::Raft::new(node_id, config, router, log_store, state_machine).await?;
///
///   // Initialize as single-node cluster
///   let mut nodes = BTreeSet::new();
///   nodes.insert(node_id);
///   raft.initialize(nodes).await?;
///
///   // Start the app loop to handle raft requests
///   let app = App::new(node_id, raft.clone(), router, state_machine);
///   tokio::spawn(async move {
///     app.run().await;
///   });
///
///   Ok(raft)
/// }
/// ```

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // Parse command line arguments
  let args: Vec<String> = std::env::args().collect();
  if args.len() < 2 {
    eprintln!("Usage: {} <transport_addr>", args[0]);
    std::process::exit(1);
  }

  let transport_addr = &args[1];

  // Initialize tracing
  tracing_subscriber::fmt::init();

  // Create configuration
  let generation = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let chitchat_id = ChitchatId::new(
    transport_addr.to_string(),
    generation,
    transport_addr.parse()?,
  );
  let config = ChitchatConfig {
    cluster_id: "chitchat-openraft-cluster".to_string(),
    chitchat_id,
    gossip_interval: Duration::from_secs(1),
    listen_addr: transport_addr.parse()?,
    seed_nodes: vec![],
    failure_detector_config: FailureDetectorConfig {
      dead_node_grace_period: Duration::from_secs(10),
      ..FailureDetectorConfig::default()
    },
    marked_for_deletion_grace_period: Duration::from_secs(60),
    catchup_callback: None,
    extra_liveness_predicate: None,
  };

  // Create and start the chitchat instance
  let chitchat_handler = spawn_chitchat(config, Vec::new(), &UdpTransport).await?;

  // TODO: Initialize OpenRaft when API compatibility is resolved
  // This is where you would call the initialize_openraft function:
  //
  // let node_id = get_node_id_from_transport(transport_addr);
  // let raft = initialize_openraft(node_id).await?;
  //
  // The key insight from the working example is that OpenRaft needs:
  // 1. Proper configuration with timeouts
  // 2. LogStore and StateMachineStore instances
  // 3. Network router for inter-node communication
  // 4. Initialization as cluster member
  // 5. Background App loop to handle raft protocol messages
  //
  // The current version uses a placeholder HashMap instead of real Raft

  // Create the hybrid application state
  let state = HybridAppState {
    chitchat: Arc::new(chitchat_handler),
    consistency_router: Arc::new(ConsistencyRouter::new()),
    stats: Arc::new(Mutex::new(ConsistencyStats::new())),
    raft_data: Arc::new(Mutex::new(HashMap::new())),
  };

  // Create Axum application
  let app = Router::new()
    .route("/", get(index))
    .route("/state", get(get_state))
    .route("/hybrid/state", get(get_hybrid_state))
    .route("/kv", post(set_kv))
    .route("/kv/v2", post(set_kv_v2))
    .route("/kv", delete(mark_for_deletion))
    .with_state(state);

  // Start the server
  let server_addr = format!("127.0.0.1:{}", get_port_from_transport(transport_addr));
  println!(
    "Starting hybrid chitchat-openraft server (simplified) on {}",
    server_addr
  );

  let listener = tokio::net::TcpListener::bind(&server_addr).await?;
  axum::serve(listener, app).await?;

  Ok(())
}

/// Extract port from transport address for the HTTP server
fn get_port_from_transport(transport_addr: &str) -> u16 {
  let parts: Vec<&str> = transport_addr.split(':').collect();
  if parts.len() == 2 {
    if let Ok(port) = parts[1].parse::<u16>() {
      port
    } else {
      8080
    }
  } else {
    8080
  }
}

/// Extract node ID from transport address (for OpenRaft)
/// In the working example, node ID is typically derived from the port or address
fn get_node_id_from_transport(transport_addr: &str) -> u64 {
  let port = get_port_from_transport(transport_addr);
  port as u64
}
