use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
  Router,
  extract::{Query, State},
  response::Json,
  routing::get,
};
use chitchat_axum_example::{
  ApiResponse, ClusterMembersResponse, ServiceUpdateResponse,
  distributed::{Cluster, Member, Service, ShardId},
};
use clap::Parser;
use cool_id_generator::Size;
use serde::Deserialize;
use tokio::{net::TcpListener, time::sleep};

#[derive(Clone)]
struct AppState {
  cluster: Arc<Cluster>,
}

#[derive(Debug, Deserialize)]
struct ServiceUpdateParams {
  service_type: String,
  host: String,
  shard: Option<u64>,
}

/// Get the current chitchat cluster state
async fn get_state(State(state): State<AppState>) -> Json<ApiResponse> {
  let cluster_state = state.cluster.cluster_state().await;
  let live_nodes = state.cluster.live_nodes().await;
  let dead_nodes = state.cluster.dead_nodes().await;

  let response = ApiResponse {
    cluster_id: "chitchat-example-cluster".to_string(),
    cluster_state,
    live_nodes,
    dead_nodes,
  };
  Json(response)
}

/// Get cluster members with their services
async fn get_members(State(state): State<AppState>) -> Json<ClusterMembersResponse> {
  let members = state.cluster.members().await;
  Json(ClusterMembersResponse { members })
}

/// Update the service of the current node
async fn update_service(
  State(state): State<AppState>,
  Query(params): Query<ServiceUpdateParams>,
) -> Json<ServiceUpdateResponse> {
  let host: SocketAddr = match params.host.parse() {
    Ok(addr) => addr,
    Err(_) => {
      return Json(ServiceUpdateResponse {
        status: false,
        message: "Invalid host format".to_string(),
      });
    }
  };

  let service = match params.service_type.as_str() {
    "searcher" => {
      let shard = params.shard.unwrap_or(0);
      Service::Searcher {
        host,
        shard: ShardId::new(shard),
      }
    }
    "api_gateway" => Service::ApiGateway { host },
    "data_processor" => {
      let shard = params.shard.unwrap_or(0);
      Service::DataProcessor {
        host,
        shard: ShardId::new(shard),
      }
    }
    "storage" => {
      let shard = params.shard.unwrap_or(0);
      Service::Storage {
        host,
        shard: ShardId::new(shard),
      }
    }
    "load_balancer" => Service::LoadBalancer { host },
    "analytics" => {
      let shard = params.shard.unwrap_or(0);
      Service::Analytics {
        host,
        shard: ShardId::new(shard),
      }
    }
    _ => {
      return Json(ServiceUpdateResponse {
        status: false,
        message: "Unknown service type".to_string(),
      });
    }
  };

  match state.cluster.set_service(service).await {
    Ok(_) => Json(ServiceUpdateResponse {
      status: true,
      message: "Service updated successfully".to_string(),
    }),
    Err(e) => Json(ServiceUpdateResponse {
      status: false,
      message: format!("Failed to update service: {}", e),
    }),
  }
}

#[derive(Debug, Parser)]
#[command(name = "chitchat", about = "Chitchat cluster example with services.")]
struct Opt {
  /// Defines the socket addr on which we should listen to.
  #[arg(long = "listen_addr", default_value = "127.0.0.1:10000")]
  listen_addr: SocketAddr,

  /// Defines the gossip address for chitchat
  #[arg(long = "gossip_addr")]
  gossip_addr: Option<SocketAddr>,

  /// Node ID. Must be unique. If None, the node ID will be generated.
  #[arg(long = "node_id")]
  node_id: Option<String>,

  /// Seed nodes for joining the cluster
  #[arg(long = "seed")]
  seeds: Vec<String>,

  /// Gossip interval in milliseconds
  #[arg(long = "interval_ms", default_value = "1000")]
  interval: u64,

  /// Service type to run on this node
  #[arg(long = "service", default_value = "api_gateway")]
  service_type: String,

  /// Shard ID for services that require sharding
  #[arg(long = "shard")]
  shard: Option<u64>,

  /// Run demo with 5 predefined nodes
  #[arg(long = "demo")]
  demo: bool,
}

fn generate_server_id(public_addr: SocketAddr) -> String {
  let cool_id = cool_id_generator::get_id(Size::Medium);
  format!("server:{public_addr}-{cool_id}")
}

fn create_service(service_type: &str, host: SocketAddr, shard: Option<u64>) -> Service {
  match service_type {
    "searcher" => Service::Searcher {
      host,
      shard: ShardId::new(shard.unwrap_or(0)),
    },
    "api_gateway" => Service::ApiGateway { host },
    "data_processor" => Service::DataProcessor {
      host,
      shard: ShardId::new(shard.unwrap_or(0)),
    },
    "storage" => Service::Storage {
      host,
      shard: ShardId::new(shard.unwrap_or(0)),
    },
    "load_balancer" => Service::LoadBalancer { host },
    "analytics" => Service::Analytics {
      host,
      shard: ShardId::new(shard.unwrap_or(0)),
    },
    _ => Service::ApiGateway { host }, // default fallback
  }
}

async fn run_demo() -> anyhow::Result<()> {
  println!("🚀 Starting chitchat cluster demo with 5 nodes...");

  // Define 5 nodes with different services
  let node_configs = vec![
    ("127.0.0.1:10001", "127.0.0.1:11001", "searcher", Some(1)),
    ("127.0.0.1:10002", "127.0.0.1:11002", "api_gateway", None),
    (
      "127.0.0.1:10003",
      "127.0.0.1:11003",
      "data_processor",
      Some(2),
    ),
    ("127.0.0.1:10004", "127.0.0.1:11004", "storage", Some(3)),
    ("127.0.0.1:10005", "127.0.0.1:11005", "analytics", Some(4)),
  ];

  let mut handles = Vec::new();

  for (i, (listen_addr, gossip_addr, service_type, shard)) in node_configs.into_iter().enumerate() {
    let listen_addr: SocketAddr = listen_addr.parse()?;
    let gossip_addr: SocketAddr = gossip_addr.parse()?;
    let service_type = service_type.to_string();

    // First node has no seeds, others connect to the first node
    let seeds = if i == 0 {
      Vec::new()
    } else {
      vec!["127.0.0.1:11001".parse()?]
    };

    let handle = tokio::spawn(async move {
      if let Err(e) = run_node(listen_addr, gossip_addr, service_type, shard, seeds).await {
        eprintln!("❌ Node {} failed: {}", i + 1, e);
      }
    });

    handles.push(handle);

    // Small delay between starting nodes
    sleep(Duration::from_millis(500)).await;
  }

  println!("✅ All nodes started! Check the cluster status at:");
  println!("   http://127.0.0.1:10001/members (Node 1 - Searcher)");
  println!("   http://127.0.0.1:10002/members (Node 2 - API Gateway)");
  println!("   http://127.0.0.1:10003/members (Node 3 - Data Processor)");
  println!("   http://127.0.0.1:10004/members (Node 4 - Storage)");
  println!("   http://127.0.0.1:10005/members (Node 5 - Analytics)");
  println!();
  println!("💡 Try updating services with:");
  println!(
    "   http://127.0.0.1:10001/update_service?service_type=searcher&host=127.0.0.1:9999&shard=99"
  );

  // Wait for all nodes
  for handle in handles {
    let _ = handle.await;
  }

  Ok(())
}

async fn run_node(
  listen_addr: SocketAddr,
  gossip_addr: SocketAddr,
  service_type: String,
  shard: Option<u64>,
  seeds: Vec<SocketAddr>,
) -> anyhow::Result<()> {
  let node_id = generate_server_id(gossip_addr);
  let service = create_service(&service_type, listen_addr, shard);
  let member = Member::with_id(node_id, service.clone());

  println!(
    "🔗 Starting node: {} on {} (gossip: {})",
    service, listen_addr, gossip_addr
  );

  let cluster = Cluster::join(member, gossip_addr, seeds).await?;
  let app_state = AppState {
    cluster: Arc::new(cluster),
  };

  let app = Router::new()
    .route("/", get(get_state))
    .route("/members", get(get_members))
    .route("/update_service", get(update_service))
    .with_state(app_state);

  let listener = TcpListener::bind(&listen_addr).await?;
  axum::serve(listener, app).await?;

  Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  tracing_subscriber::fmt::init();

  let opt = Opt::parse();

  if opt.demo {
    return run_demo().await;
  }

  let listen_addr = opt.listen_addr;
  let gossip_addr = opt.gossip_addr.unwrap_or(listen_addr);
  let node_id = opt
    .node_id
    .unwrap_or_else(|| generate_server_id(gossip_addr));

  let service = create_service(&opt.service_type, listen_addr, opt.shard);
  let member = Member::with_id(node_id, service.clone());

  let seeds: Vec<SocketAddr> = opt.seeds.iter().filter_map(|s| s.parse().ok()).collect();

  println!(
    "🔗 Starting node: {} on {} (gossip: {})",
    service, listen_addr, gossip_addr
  );

  let cluster = Cluster::join(member, gossip_addr, seeds).await?;
  let app_state = AppState {
    cluster: Arc::new(cluster),
  };

  let app = Router::new()
    .route("/", get(get_state))
    .route("/members", get(get_members))
    .route("/update_service", get(update_service))
    .with_state(app_state);

  println!("🌐 API server listening on {}", listen_addr);
  println!("📡 Gossip protocol running on {}", gossip_addr);

  let listener = TcpListener::bind(&listen_addr).await?;
  axum::serve(listener, app).await?;

  Ok(())
}
