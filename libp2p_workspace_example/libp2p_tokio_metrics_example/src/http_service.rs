use hyper::http::StatusCode;
use hyper::service::Service;
use hyper::{Body, Method, Request, Response, Server};
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

const METRICS_CONTENT_TYPE: &str = "application/openmetrics-text;charset=utf-8;version=1.0.0";

pub(crate) async fn metrics_server(registry: Registry) -> Result<(), std::io::Error> {
  // Serve on localhost.
  let addr = ([127, 0, 0, 1], 0).into();

  let server = Server::bind(&addr).serve(MakeMetricService::new(registry));
  tracing::info!(metrics_server=%format!("http://{}/metrics", server.local_addr()));
  if let Err(e) = server.await {
    tracing::error!("server error: {}", e);
  }
  Ok(())
}

type SharedRegistry = Arc<Mutex<Registry>>;

pub(crate) struct MetricService {
  reg: SharedRegistry,
}

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
    encode(&mut response.body_mut(), &reg.lock().unwrap()).unwrap();

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

impl Service<Request<Body>> for MetricService {
  type Response = Response<String>;
  type Error = hyper::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, req: Request<Body>) -> Self::Future {
    let req_path = req.uri().path();
    let req_method = req.method();
    let resp = if (req_method == Method::GET) && (req_path == "/metrics") {
      // Encode and serve metrics from registry.
      self.respond_with_metrics()
    } else {
      self.respond_with_404_not_found()
    };
    Box::pin(async { Ok(resp) })
  }
}

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

impl<T> Service<T> for MakeMetricService {
  type Response = MetricService;
  type Error = hyper::Error;
  type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

  fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
    Poll::Ready(Ok(()))
  }

  fn call(&mut self, _: T) -> Self::Future {
    let reg = self.reg.clone();
    let fut = async move { Ok(MetricService { reg }) };
    Box::pin(fut)
  }
}
