use hyper::http::StatusCode;
use hyper::server::conn::http1;
use hyper::service::Service;
use hyper::{body::Incoming, Method, Request, Response};
use hyper_util::rt::TokioIo;
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;

const METRICS_CONTENT_TYPE: &str = "application/openmetrics-text;charset=utf-8;version=1.0.0";

pub(crate) async fn metrics_server(registry: Registry) -> Result<(), std::io::Error> {
  // Serve on localhost.
  let addr: SocketAddr = ([127, 0, 0, 1], 0).into();
  tracing::info!(metrics_server=%format!("http://{:?}/metrics", addr));
  let make_metrics_service = MakeMetricService::new(registry);
  let listener = TcpListener::bind(addr).await?;
  loop {
    let (stream, _) = listener.accept().await?;
    let io = TokioIo::new(stream);
    let make_metrics_service_clone = make_metrics_service.clone();
    tokio::task::spawn(async move {
      if let Err(err) = http1::Builder::new()
        .serve_connection(io, make_metrics_service_clone)
        .await
      {
        tracing::error!("server error: {}", err);
      }
    });
  }
}

#[derive(Debug, Clone)]
pub(crate) struct MetricService {
  reg: SharedRegistry,
}

type SharedRegistry = Arc<Mutex<Registry>>;

impl MetricService {
  fn get_reg(&mut self) -> SharedRegistry {
    Arc::clone(&self.reg)
  }
  fn respond_with_metrics(&mut self) -> Response<String> {
    let mut response: Response<String> = Response::default();

    response.headers_mut().insert(
      hyper::header::CONTENT_TYPE,
      METRICS_CONTENT_TYPE.try_into().unwrap(),
    );

    let reg = self.get_reg();
    let mut inner_str = String::new();
    encode(&mut inner_str, &reg.lock().unwrap()).unwrap();
    *response.body_mut() = inner_str;

    *response.status_mut() = StatusCode::OK;

    response
  }
  fn respond_with_404_not_found(&mut self) -> Response<String> {
    Response::builder()
      .status(StatusCode::NOT_FOUND)
      .body("Not found try localhost:[port]/metrics".to_string())
      .unwrap()
  }
}

impl Service<Request<Incoming>> for MetricService {
  type Response = Response<String>;
  type Error = hyper::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn call(&self, req: Request<Incoming>) -> Self::Future {
    let req_path = req.uri().path();
    let req_method = req.method();
    let resp = if (req_method == Method::GET) && (req_path == "/metrics") {
      // Encode and serve metrics from registry.
      self.clone().respond_with_metrics()
    } else {
      self.clone().respond_with_404_not_found()
    };
    Box::pin(async { Ok(resp) })
  }
}

#[derive(Debug, Clone)]
pub(crate) struct MakeMetricService {
  reg: SharedRegistry,
}

impl MakeMetricService {
  pub(crate) fn new(registry: Registry) -> MakeMetricService {
    MakeMetricService {
      reg: Arc::new(Mutex::new(registry)),
    }
  }
}

impl Service<Request<Incoming>> for MakeMetricService {
  type Response = Response<String>;
  type Error = hyper::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn call(&self, req: Request<Incoming>) -> Self::Future {
    let reg = self.reg.clone();
    let fut = async move { Ok(MetricService { reg }.call(req).await.unwrap()) };
    Box::pin(fut)
  }
}
