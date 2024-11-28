use std::{
  net::SocketAddr,
  sync::{Arc, Mutex},
};

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::get, Router};
use prometheus_client::{encoding::text::encode, registry::Registry};
use tokio::net::TcpListener;

const METRICS_CONTENT_TYPE: &str = "application/openmetrics-text;charset=utf-8;version=1.0.0";
pub(crate) async fn metrics_server(
  registry: Registry,
  metrics_path: String,
) -> Result<(), std::io::Error> {
  // Serve on localhost.
  let addr: SocketAddr = ([0, 0, 0, 0], 8888).into();
  let service = MetricService::new(registry);
  let server = Router::new()
    .route(&metrics_path, get(respond_with_metrics))
    .with_state(service);
  let tcp_listener = TcpListener::bind(addr).await?;
  let local_addr = tcp_listener.local_addr()?;
  tracing::info!(metrics_server=%format!("http://{}{}", local_addr, metrics_path));
  axum::serve(tcp_listener, server.into_make_service()).await?;
  Ok(())
}

async fn respond_with_metrics(state: State<MetricService>) -> impl IntoResponse {
  let mut sink = String::new();
  let reg = state.get_reg();
  encode(&mut sink, &reg.lock().unwrap()).unwrap();

  (
    StatusCode::OK,
    [(axum::http::header::CONTENT_TYPE, METRICS_CONTENT_TYPE)],
    sink,
  )
}

#[derive(Clone)]
pub(crate) struct MetricService {
  reg: Arc<Mutex<Registry>>,
}

type SharedRegistry = Arc<Mutex<Registry>>;

impl MetricService {
  fn new(reg: Registry) -> Self {
    Self {
      reg: Arc::new(Mutex::new(reg)),
    }
  }

  fn get_reg(&self) -> SharedRegistry {
    Arc::clone(&self.reg)
  }
}
