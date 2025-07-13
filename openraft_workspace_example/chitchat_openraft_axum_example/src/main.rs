use std::{
  net::SocketAddr,
  sync::Arc,
  time::{Duration, SystemTime},
};

use axum::{
  Router,
  extract::{Query, State},
  response::Json,
  routing::get,
};
use chitchat::{
  Chitchat, ChitchatConfig, ChitchatId, FailureDetectorConfig, spawn_chitchat,
  transport::UdpTransport,
};
use chitchat_openraft_axum_example::{ApiResponse, SetKeyValueResponse};
use cool_id_generator::Size;
use serde::Deserialize;
use structopt::StructOpt;
use tokio::{net::TcpListener, sync::Mutex};

#[derive(Clone)]
struct AppState {
  chitchat: Arc<Mutex<Chitchat>>,
}

#[derive(Debug, Deserialize)]
struct SetKeyValueParams {
  key: String,
  value: String,
}

#[derive(Debug, Deserialize)]
struct DeleteKeyParams {
  key: String,
}

/// Get the current chitchat cluster state
async fn get_state(State(state): State<AppState>) -> Json<ApiResponse> {
  let chitchat_guard = state.chitchat.lock().await;
  let response = ApiResponse {
    cluster_id: chitchat_guard.cluster_id().to_string(),
    cluster_state: chitchat_guard.state_snapshot(),
    live_nodes: chitchat_guard.live_nodes().cloned().collect::<Vec<_>>(),
    dead_nodes: chitchat_guard.dead_nodes().cloned().collect::<Vec<_>>(),
  };
  Json(response)
}

/// Set a key-value pair on this node
async fn set_kv(
  State(state): State<AppState>,
  Query(params): Query<SetKeyValueParams>,
) -> Json<SetKeyValueResponse> {
  let mut chitchat_guard = state.chitchat.lock().await;

  let cc_state = chitchat_guard.self_node_state();
  cc_state.set(&params.key, &params.value);

  Json(SetKeyValueResponse { status: true })
}

/// Mark a key for deletion on this node
async fn mark_for_deletion(
  State(state): State<AppState>,
  Query(params): Query<DeleteKeyParams>,
) -> Json<SetKeyValueResponse> {
  let mut chitchat_guard = state.chitchat.lock().await;

  let cc_state = chitchat_guard.self_node_state();
  cc_state.delete(&params.key);

  Json(SetKeyValueResponse { status: true })
}

#[derive(Debug, StructOpt)]
#[structopt(name = "chitchat", about = "Chitchat test server.")]
struct Opt {
  /// Defines the socket addr on which we should listen to.
  #[structopt(long = "listen_addr", default_value = "127.0.0.1:10000")]
  listen_addr: SocketAddr,
  /// Defines the socket address (host:port) other servers should use to
  /// reach this server.
  ///
  /// It defaults to the listen address, but this is only valid
  /// when all server are running on the same server.
  #[structopt(long = "public_addr")]
  public_addr: Option<SocketAddr>,

  /// Node ID. Must be unique. If None, the node ID will be generated from
  /// the public_addr and a random suffix.
  #[structopt(long = "node_id")]
  node_id: Option<String>,

  #[structopt(long = "seed")]
  seeds: Vec<String>,

  #[structopt(long = "interval_ms", default_value = "500")]
  interval: u64,
}

fn generate_server_id(public_addr: SocketAddr) -> String {
  let cool_id = cool_id_generator::get_id(Size::Medium);
  format!("server:{public_addr}-{cool_id}")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  tracing_subscriber::fmt::init();

  let opt = Opt::from_args();
  let public_addr = opt.public_addr.unwrap_or(opt.listen_addr);
  let node_id = opt
    .node_id
    .unwrap_or_else(|| generate_server_id(public_addr));
  let generation = SystemTime::now()
    .duration_since(SystemTime::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let chitchat_id = ChitchatId::new(node_id, generation, public_addr);
  let config = ChitchatConfig {
    cluster_id: "testing".to_string(),
    chitchat_id,
    gossip_interval: Duration::from_millis(opt.interval),
    listen_addr: opt.listen_addr,
    seed_nodes: opt.seeds.clone(),
    failure_detector_config: FailureDetectorConfig {
      dead_node_grace_period: Duration::from_secs(10),
      ..FailureDetectorConfig::default()
    },
    marked_for_deletion_grace_period: Duration::from_secs(60),
    catchup_callback: None,
    extra_liveness_predicate: None,
  };

  let chitchat_handler = spawn_chitchat(config, Vec::new(), &UdpTransport).await?;
  let chitchat = chitchat_handler.chitchat();

  let app_state = AppState { chitchat };

  let app = Router::new()
    .route("/", get(get_state))
    .route("/set_kv", get(set_kv))
    .route("/mark_for_deletion", get(mark_for_deletion))
    .with_state(app_state);

  println!("Chitchat API server starting on {}", opt.listen_addr);

  let listener = TcpListener::bind(&opt.listen_addr).await?;
  axum::serve(listener, app).await?;

  Ok(())
}
