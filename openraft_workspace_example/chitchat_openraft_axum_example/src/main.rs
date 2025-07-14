use std::{net::SocketAddr, sync::Arc};

use aide::openapi::{Info, OpenApi};
use axum::Extension;
use chitchat_openraft_axum_example::{
  api::AppState,
  cli::Opt,
  demo::run_demo,
  distributed::{Cluster, Member},
  router::create_router,
  utils::{create_service, generate_server_id},
};
use clap::Parser;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

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
  let member = Member::with_id(node_id.clone(), service.clone());

  let seeds: Vec<SocketAddr> = opt.seeds.iter().filter_map(|s| s.parse().ok()).collect();

  println!(
    "🔗 Starting node: {} on {} (gossip: {})",
    service, listen_addr, gossip_addr
  );

  let cluster = Cluster::join(member, gossip_addr, seeds).await?;

  // Enable OpenRAFT integration
  let raft_node_id = format!("raft-{}", node_id);
  if let Err(e) = cluster.enable_raft(raft_node_id).await {
    tracing::warn!("Failed to enable OpenRAFT: {:?}", e);
  } else {
    tracing::info!("OpenRAFT integration enabled");
  }

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

  println!("🌐 API server listening on {}", listen_addr);
  println!("📡 Gossip protocol running on {}", gossip_addr);
  println!("📚 API documentation available at:");
  println!("   http://{}/docs/scalar (Scalar UI)", listen_addr);
  println!("   http://{}/docs/swagger (Swagger UI)", listen_addr);
  println!("   http://{}/docs/redoc (Redoc)", listen_addr);
  println!("   http://{}/docs (Documentation Index)", listen_addr);

  let listener = TcpListener::bind(&listen_addr).await?;
  axum::serve(listener, app).await?;

  Ok(())
}
