use std::{net::SocketAddr, sync::Arc, time::Duration};

use aide::openapi::{Info, OpenApi};
use axum::Extension;
use base64::Engine;
use tokio::{net::TcpListener, time::sleep};
use tower_http::cors::CorsLayer;

use crate::{
  api::AppState,
  distributed::{Cluster, Member},
  router::create_router,
  utils::{create_service, generate_server_id},
};

pub async fn run_demo() -> anyhow::Result<()> {
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
  println!("📚 API documentation available at:");
  println!("   http://127.0.0.1:10001/docs/swagger (Swagger UI)");
  println!("   http://127.0.0.1:10001/docs/scalar (Scalar UI)");
  println!("   http://127.0.0.1:10001/docs/redoc (Redoc)");
  println!();
  println!("🔧 OpenRAFT Store Endpoints:");
  println!("   http://127.0.0.1:10001/raft/store/html (HTML Store View)");
  println!("   http://127.0.0.1:10001/raft/store (JSON Store API)");
  println!("   http://127.0.0.1:10001/raft/tables (List Tables)");
  println!();
  println!("💡 Try updating services with:");
  println!(
    "   curl -X POST http://127.0.0.1:10001/update_service -H 'Content-Type: application/json' -d \
     '{{\"service_type\":\"searcher\",\"host\":\"127.0.0.1:9999\",\"shard\":99}}'"
  );
  println!();
  println!("💡 Try setting a key-value pair:");
  println!(
    "   curl -X POST http://127.0.0.1:10001/raft/set -H 'Content-Type: application/json' -d \
     '{{\"table\":\"test\",\"key\":\"hello\",\"value\":\"{}\"}}'",
    base64::prelude::BASE64_STANDARD.encode("world")
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

  let mut api = OpenApi {
    info: Info {
      title: "Chitchat Cluster API".to_string(),
      version: "1.0.0".to_string(),
      description: Some("API for managing chitchat cluster nodes and services".to_string()),
      ..Default::default()
    },
    ..Default::default()
  };

  let app = create_router()
    .finish_api(&mut api)
    .layer(Extension(api))
    .layer(CorsLayer::permissive())
    .with_state(app_state);

  let listener = TcpListener::bind(&listen_addr).await?;
  axum::serve(listener, app).await?;

  Ok(())
}
