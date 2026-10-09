//! Axum owns the HTTP server; Topcoat owns pages and the encrypted business gateway.
use axum::{
  Json, Router,
  extract::Request,
  middleware::{self, Next},
  response::Response,
  routing::get,
};
use topcoat::router::tower::TowerService;

pub fn app() -> Router {
  // Build once: TowerService clones share the same Topcoat app contexts and sessions.
  let topcoat = TowerService::new(crate::secure::wrap(crate::router()));
  Router::new()
    .route("/healthz", get(health))
    // Keep the complete URI, including /pq and /assets, for Topcoat routing.
    // Mount the public gateway, never the unprotected business router.
    .fallback_service(topcoat)
    .layer(middleware::from_fn(common_headers))
}

async fn health() -> Json<serde_json::Value> {
  // Liveness only; no business state is exposed outside the encrypted channel.
  Json(serde_json::json!({"status": "ok"}))
}

async fn common_headers(request: Request, next: Next) -> Response {
  let mut response = next.run(request).await;
  response
    .headers_mut()
    .insert("x-content-type-options", "nosniff".parse().unwrap());
  response
    .headers_mut()
    .insert("cache-control", "no-store".parse().unwrap());
  response
}

#[cfg(test)]
mod tests {
  use axum::{
    body::{Body, to_bytes},
    http::Request,
  };
  use topcoat_gpui_protocol::{
    CounterSnapshot,
    crypto::{self, ApiRequest, ApiResponse, ClientHandshake, ServerHello},
  };
  use tower::ServiceExt;

  use super::*;

  async fn send(app: &Router, method: &str, path: &str, body: String) -> (u16, Vec<u8>) {
    let response = app
      .clone()
      .oneshot(
        Request::builder()
          .method(method)
          .uri(path)
          .header("content-type", "application/json")
          .body(Body::from(body))
          .unwrap(),
      )
      .await
      .unwrap();
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.headers()["cache-control"], "no-store");
    let status = response.status().as_u16();
    (
      status,
      to_bytes(response.into_body(), crypto::MAX_WIRE)
        .await
        .unwrap()
        .to_vec(),
    )
  }

  async fn encrypted(app: &Router, method: &str, body: &str) -> ApiResponse {
    let (client, hello) = ClientHandshake::begin().unwrap();
    let (status, bytes) = send(
      app,
      "POST",
      crypto::HANDSHAKE_PATH,
      serde_json::to_string(&hello).unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    let hello: ServerHello = serde_json::from_slice(&bytes).unwrap();
    let (envelope, reader) = client
      .finish(&hello, &hello.public_key)
      .unwrap()
      .seal(
        &serde_json::to_vec(&ApiRequest {
          method: method.into(),
          path: "/api/counter".into(),
          body: body.into(),
          form: false,
        })
        .unwrap(),
      )
      .unwrap();
    let (status, bytes) = send(
      app,
      "POST",
      crypto::EXCHANGE_PATH,
      serde_json::to_string(&envelope).unwrap(),
    )
    .await;
    assert_eq!(status, 200);
    serde_json::from_slice(
      &reader
        .open(&serde_json::from_slice(&bytes).unwrap())
        .unwrap(),
    )
    .unwrap()
  }

  #[tokio::test]
  async fn axum_routes_and_topcoat_fallback_share_middleware() {
    let app = app();
    let (status, bytes) = send(&app, "GET", "/healthz", String::new()).await;
    assert_eq!(status, 200);
    assert_eq!(
      serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
      serde_json::json!({"status":"ok"})
    );
    for (path, marker) in [
      ("/", "Topcoat"),
      ("/studio", "配色实验室"),
      ("/app.js", "/api/counter"),
      ("/assets/studio", "/api/studio"),
    ] {
      let (status, bytes) = send(&app, "GET", path, String::new()).await;
      assert_eq!(status, 200, "{path}");
      assert!(std::str::from_utf8(&bytes).unwrap().contains(marker));
    }
    for method in ["GET", "POST"] {
      assert_eq!(
        send(&app, method, "/api/counter", String::new()).await.0,
        403
      );
    }
    assert_eq!(send(&app, "POST", "/healthz", String::new()).await.0, 405);
  }

  #[tokio::test]
  async fn encrypted_clients_share_state_through_axum() {
    let app = app();
    for (method, body, value) in [("POST", r#"{"action":"increment"}"#, 1), ("GET", "", 1)] {
      let response = encrypted(&app, method, body).await;
      assert_eq!(response.status, 200);
      assert_eq!(
        serde_json::from_str::<CounterSnapshot>(&response.body).unwrap(),
        CounterSnapshot { value, revision: 1 }
      );
    }
    assert_eq!(
      encrypted(&app, "POST", r#"{"action":"bad"}"#).await.status,
      400
    );
    assert_eq!(
      send(&app, "POST", crypto::HANDSHAKE_PATH, "not json".into())
        .await
        .0,
      400
    );
  }
}
