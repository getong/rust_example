use std::{net::SocketAddr, sync::Arc};

use chitchat_axum_example::{
  api::AppState,
  cli::Opt,
  demo::run_demo,
  distributed::{Cluster, Member},
  router::create_router,
  utils::{create_service, generate_server_id},
};
use clap::Parser;
use tokio::net::TcpListener;

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

  let app = create_router().with_state(app_state);

  println!("🌐 API server listening on {}", listen_addr);
  println!("📡 Gossip protocol running on {}", gossip_addr);

  let listener = TcpListener::bind(&listen_addr).await?;
  axum::serve(listener, app).await?;

  Ok(())
}
