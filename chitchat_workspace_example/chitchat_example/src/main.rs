use std::{
  net::SocketAddr,
  sync::Arc,
  time::{Duration, SystemTime},
};

use chitchat::{
  spawn_chitchat, transport::UdpTransport, Chitchat, ChitchatConfig, ChitchatId,
  ClusterStateSnapshot, FailureDetectorConfig,
};
use clap::Parser;
use cool_id_generator::Size;
use poem::{listener::TcpListener, Route, Server};
use poem_openapi::{param::Query, payload::Json, OpenApi, OpenApiService};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse {
  pub cluster_id: String,
  pub cluster_state: ClusterStateSnapshot,
  pub live_nodes: Vec<ChitchatId>,
  pub dead_nodes: Vec<ChitchatId>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SetKeyValueResponse {
  pub status: bool,
}

struct Api {
  chitchat: Arc<Mutex<Chitchat>>,
}

#[OpenApi]
impl Api {
  /// Chitchat state
  #[oai(path = "/", method = "get")]
  async fn index(&self) -> Json<serde_json::Value> {
    let chitchat_guard = self.chitchat.lock().await;
    let response = ApiResponse {
      cluster_id: chitchat_guard.cluster_id().to_string(),
      cluster_state: chitchat_guard.state_snapshot(),
      live_nodes: chitchat_guard.live_nodes().cloned().collect::<Vec<_>>(),
      dead_nodes: chitchat_guard.dead_nodes().cloned().collect::<Vec<_>>(),
    };
    Json(serde_json::to_value(&response).unwrap())
  }

  /// Sets a key-value pair on this node (without validation).
  #[oai(path = "/set_kv/", method = "get")]
  async fn set_kv(&self, key: Query<String>, value: Query<String>) -> Json<serde_json::Value> {
    let mut chitchat_guard = self.chitchat.lock().await;

    let cc_state = chitchat_guard.self_node_state();
    cc_state.set(key.as_str(), value.as_str());

    Json(serde_json::to_value(&SetKeyValueResponse { status: true }).unwrap())
  }

  /// Marks a key for deletion on this node (without validation).
  #[oai(path = "/mark_for_deletion/", method = "get")]
  async fn mark_for_deletion(&self, key: Query<String>) -> Json<serde_json::Value> {
    let mut chitchat_guard = self.chitchat.lock().await;

    let cc_state = chitchat_guard.self_node_state();
    cc_state.delete(key.as_str());
    Json(serde_json::to_value(&SetKeyValueResponse { status: true }).unwrap())
  }
}

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Opt {
  /// Defines the socket addr on which we should listen to.
  #[arg(long = "listen_addr", default_value = "127.0.0.1:10000")]
  listen_addr: SocketAddr,
  /// Defines the socket address (host:port) other servers should use to
  /// reach this server.
  ///
  /// It defaults to the listen address, but this is only valid
  /// when all server are running on the same server.
  #[arg(long = "public_addr")]
  public_addr: Option<SocketAddr>,

  /// Node ID. Must be unique. If None, the node ID will be generated from
  /// the public_addr and a random suffix.
  #[arg(long = "node_id")]
  node_id: Option<String>,

  #[arg(long = "seed")]
  seeds: Vec<String>,

  #[arg(long, default_value_t = 500)]
  interval: u64,
}

fn generate_server_id(public_addr: SocketAddr) -> String {
  let cool_id = cool_id_generator::get_id(Size::Medium);
  format!("server:{public_addr}-{cool_id}")
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  tracing_subscriber::fmt::init();
  let opt = Opt::parse();
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
  let api = Api { chitchat };
  let api_service =
    OpenApiService::new(api, "Hello World", "1.0").server(format!("http://{}/", opt.listen_addr));
  let docs = api_service.swagger_ui();
  let app = Route::new().nest("/", api_service).nest("/docs", docs);
  Server::new(TcpListener::bind(&opt.listen_addr))
    .run(app)
    .await?;
  Ok(())
}
