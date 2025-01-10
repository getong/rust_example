use std::{convert::Infallible, sync::Arc};

use async_trait::async_trait;
use axum::{
  Router,
  body::Body,
  http::{Request, Response, StatusCode},
  response::IntoResponse,
  routing::get,
};
use tokio::net::TcpListener;
use tower::util::BoxCloneSyncService;

// Define a simple state to pass to our service
#[derive(Clone)]
pub struct AppState {
  message: String,
}

// Create a simple service
#[derive(Clone)]
struct MyService {
  state: Arc<AppState>,
}

#[async_trait]
impl tower::Service<Request<Body>> for MyService {
  type Response = Response<Body>;
  type Error = Infallible;
  type Future = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
  >;

  fn poll_ready(&mut self, _: &mut std::task::Context) -> std::task::Poll<Result<(), Self::Error>> {
    std::task::Poll::Ready(Ok(()))
  }

  fn call(&mut self, _req: Request<Body>) -> Self::Future {
    let response = Response::new(Body::from(self.state.message.clone()));
    Box::pin(async { Ok(response) })
  }
}

// Define a struct that implements the Service trait for hostname routing
#[derive(Clone)]
struct HostnameService {
  state: Arc<AppState>,
}

#[async_trait]
impl tower::Service<Request<Body>> for HostnameService {
  type Response = Response<Body>;
  type Error = Infallible;
  type Future = std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
  >;

  fn poll_ready(&mut self, _: &mut std::task::Context) -> std::task::Poll<Result<(), Self::Error>> {
    std::task::Poll::Ready(Ok(()))
  }

  fn call(&mut self, req: Request<Body>) -> Self::Future {
    let state = self.state.clone();
    let hostname = req
      .headers()
      .get("host")
      .and_then(|v| v.to_str().ok())
      .unwrap_or("")
      .to_string();

    Box::pin(async move {
      if hostname == "localhost" || hostname == "127.0.0.1" || hostname == "[::1]" {
        let mut service = MyService { state };
        service.call(req).await
      } else {
        Ok(StatusCode::NOT_FOUND.into_response())
      }
    })
  }
}

// The `BoxCloneSyncService` example
pub fn mk_hostname_router(
  state: Arc<AppState>,
) -> BoxCloneSyncService<Request<Body>, Response<Body>, Infallible> {
  BoxCloneSyncService::new(HostnameService { state })
}

#[tokio::main]
async fn main() {
  let state = Arc::new(AppState {
    message: "Hello, world!".to_string(),
  });

  // Use fallback_service instead of nest_service
  let hostname_router = mk_hostname_router(state);

  let app = Router::new()
    .route("/", get(|| async { "Hello, world!" }))
    .nest_service("/other", hostname_router);

  // Start the Axum server on port 3000
  let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, app).await.unwrap();
}
