use std::{net::SocketAddr, sync::Arc};

use aide::openapi::{Info, OpenApi};
use axum::Extension;
use chitchat_openraft_axum_example::{
  api::AppState,
  cli::Opt,
  demo::run_demo,
  distributed::{Cluster, Member, cluster::init_file_logging},
  router::create_router,
  utils::{create_service, generate_server_id},
};
use clap::Parser;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  // Print current directory for debugging
  println!(
    "Starting application in directory: {:?}",
    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
  );

  // Initialize file logging first
  match init_file_logging() {
    Ok(_) => println!("✅ File logging initialized successfully"),
    Err(e) => {
      eprintln!("⚠️  Warning: Failed to initialize file logging: {}", e);
      eprintln!("   Falling back to console-only logging");
      tracing_subscriber::fmt::init();
    }
  }

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

  tracing::info!("Attempting to bind to address: {}", listen_addr);

  // Try to bind to the address first to give a better error message
  let listener = match TcpListener::bind(&listen_addr).await {
    Ok(listener) => {
      tracing::info!("Successfully bound to address: {}", listen_addr);
      listener
    }
    Err(e) => {
      tracing::error!("Failed to bind to address {}: {}", listen_addr, e);
      eprintln!("❌ Error: Cannot bind to address {}", listen_addr);
      eprintln!("   Reason: {}", e);

      if e.kind() == std::io::ErrorKind::AddrInUse {
        eprintln!("💡 Suggestion: The address is already in use. Try:");
        eprintln!("   - Using a different port: --listen_addr 127.0.0.1:10001");
        eprintln!("   - Stopping other processes using this port");
        eprintln!("   - Waiting a moment and trying again");

        // Find next available port
        for port in (listen_addr.port() + 1) ..= (listen_addr.port() + 10) {
          let test_addr = SocketAddr::new(listen_addr.ip(), port);
          if TcpListener::bind(&test_addr).await.is_ok() {
            eprintln!("   - Available port found: {}", test_addr);
            break;
          }
        }
      }

      return Err(e.into());
    }
  };

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
  println!("📄 Logs are being written to: ./logs/chitchat_cluster.log");

  tracing::info!("Starting axum server on {}", listen_addr);

  // Add graceful shutdown handling
  let result = axum::serve(listener, app)
    .with_graceful_shutdown(shutdown_signal())
    .await;

  if let Err(e) = result {
    tracing::error!("Server error: {}", e);
    return Err(e.into());
  }

  tracing::info!("Server shut down gracefully");
  Ok(())
}

async fn shutdown_signal() {
  let ctrl_c = async {
    tokio::signal::ctrl_c()
      .await
      .expect("failed to install Ctrl+C handler");
  };

  #[cfg(unix)]
  let terminate = async {
    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
      .expect("failed to install signal handler")
      .recv()
      .await;
  };

  #[cfg(not(unix))]
  let terminate = std::future::pending::<()>();

  tokio::select! {
    _ = ctrl_c => {
      tracing::info!("Received Ctrl+C, shutting down gracefully");
    },
    _ = terminate => {
      tracing::info!("Received terminate signal, shutting down gracefully");
    },
  }
}
