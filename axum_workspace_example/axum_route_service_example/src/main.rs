use axum::{body::Body, extract::Request, routing::any_service, Router};
use http::Response;
use std::convert::Infallible;
use tower::service_fn;
use tower_http::services::ServeFile;

// curl http://localhost:3000/foo
// curl http://localhost:3000/static/Cargo.toml
#[tokio::main]
async fn main() {
  let app = Router::new()
    .route(
      // Any request to `/` goes to a service
      "/",
      // Services whose response body is not `axum::body::BoxBody`
      // can be wrapped in `axum::routing::any_service` (or one of the other routing filters)
      // to have the response body mapped
      any_service(service_fn(|_: Request| async {
        let res = Response::new(Body::from("Hi from `GET /`"));
        Ok::<_, Infallible>(res)
      })),
    )
    .route_service(
      "/foo",
      // This service's response body is `axum::body::BoxBody` so
      // it can be routed to directly.
      service_fn(|req: Request| async move {
        let body = Body::from(format!("Hi from `{} /foo`", req.method()));
        let res = Response::new(body);
        Ok::<_, Infallible>(res)
      }),
    )
    .route_service(
      // GET `/static/Cargo.toml` goes to a service from tower-http
      "/static/Cargo.toml",
      ServeFile::new("Cargo.toml"),
    );

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, app).await.unwrap();
}
