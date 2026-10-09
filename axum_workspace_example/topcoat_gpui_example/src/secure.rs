//! Public encrypted gateway. The business router is reachable only after AEAD authentication.
use std::{
  collections::HashMap,
  time::{Duration, Instant},
};

use tokio::sync::Mutex;
use topcoat::{
  Result,
  context::{Cx, app_context},
  router::{
    self, Body, Layer, LayerFuture, Next, Router,
    content::Json,
    request::{self, Request},
    response::Response,
    route, to_bytes,
  },
};
use topcoat_gpui_protocol::crypto::{
  self, ApiRequest, ApiResponse, ClientHello, Envelope, Identity, ServerChannel, ServerHello,
};
const TTL: Duration = Duration::from_secs(60);
const CAPACITY: usize = 1024;
struct Gateway {
  sessions: Mutex<HashMap<String, (Instant, ServerChannel)>>,
  business: Router,
}
fn reject() -> topcoat::Error {
  router::error::bad_request("Invalid or expired encrypted exchange").into()
}

pub fn wrap(business: Router) -> Router {
  Router::builder()
    .app_context(Gateway {
      sessions: Mutex::new(HashMap::new()),
      business,
    })
    .route(handshake)
    .route(exchange)
    .layer(PublicBoundary)
    .build()
}
struct PublicBoundary;
impl Layer for PublicBoundary {
  fn path(&self) -> Option<&router::Path> {
    None
  }
  fn handle<'a>(&'a self, cx: &'a Cx, body: Body, next: Next<'a>) -> LayerFuture<'a> {
    Box::pin(async move {
      let path = request::uri(cx).path();
      let mut response = if matches!(path, "/pq/handshake" | "/pq/exchange") {
        let limit = if path == "/pq/exchange" {
          crypto::MAX_WIRE
        } else {
          8192
        };
        let bytes = to_bytes(body, limit).await.map_err(|_| reject())?;
        next.run(cx, Body::from(bytes)).await?
      } else if request::method(cx) == "GET"
        && matches!(
          path,
          "/"
            | "/todos"
            | "/echo"
            | "/profile"
            | "/studio"
            | "/app.js"
            | "/demos.js"
            | "/assets/studio"
        )
      {
        app_context::<Gateway>(cx)
          .business
          .handle(Request::from_parts(
            request::parts(cx).clone(),
            Body::empty(),
          ))
          .await
      } else {
        Response::builder()
          .status(403)
          .body(Body::from("Use the post-quantum encrypted gateway"))
          .unwrap()
      };
      response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
      response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
      Ok(response)
    })
  }
}
#[route(POST "/pq/handshake")]
async fn handshake(cx: &Cx, Json(hello): Json<ClientHello>) -> Result<Json<ServerHello>> {
  let gateway = app_context::<Gateway>(cx);
  let mut sessions = gateway.sessions.lock().await;
  sessions.retain(|_, (created, _)| created.elapsed() < TTL);
  if sessions.len() >= CAPACITY {
    return Err(router::error::too_many_requests(60).into());
  }
  let (reply, channel) = Identity::generate()
    .map_err(|_| reject())?
    .accept(&hello)
    .map_err(|_| reject())?;
  sessions.insert(reply.session.clone(), (Instant::now(), channel));
  Ok(Json(reply))
}
#[route(POST "/pq/exchange")]
async fn exchange(cx: &Cx, Json(envelope): Json<Envelope>) -> Result<Json<Envelope>> {
  let gateway = app_context::<Gateway>(cx);
  // Atomically remove before dispatch. Replays and concurrent duplicates cannot mutate state twice.
  let (created, channel) = gateway
    .sessions
    .lock()
    .await
    .remove(&envelope.session)
    .ok_or_else(reject)?;
  if created.elapsed() >= TTL {
    return Err(reject());
  }
  let (plaintext, writer) = channel.open(&envelope).map_err(|_| reject())?;
  let input: ApiRequest = serde_json::from_slice(&plaintext).map_err(|_| reject())?;
  let allowed = matches!(
    (input.method.as_str(), input.path.as_str()),
    ("GET", "/api/counter" | "/api/demos" | "/api/studio")
      | (
        "POST",
        "/api/counter"
          | "/api/todos"
          | "/api/echo"
          | "/api/profile"
          | "/api/studio"
          | "/api/suggestions"
      )
  );
  let output = if !allowed || input.form || (input.method == "GET" && !input.body.is_empty()) {
    ApiResponse {
      status: 400,
      body: "Unsupported encrypted API request".into(),
    }
  } else {
    let request = Request::builder()
      .method(input.method.as_str())
      .uri(&input.path)
      .header("content-type", "application/json")
      .body(Body::from(input.body))
      .map_err(|_| reject())?;
    let response = gateway.business.handle(request).await;
    let status = response.status().as_u16();
    match to_bytes(response.into_body(), crypto::MAX_PLAINTEXT / 2).await {
      Ok(bytes) => ApiResponse {
        status,
        body: String::from_utf8(bytes.to_vec()).map_err(|_| reject())?,
      },
      Err(_) => ApiResponse {
        status: 500,
        body: "Response exceeds encrypted channel limit".into(),
      },
    }
  };
  let bytes = serde_json::to_vec(&output).map_err(|_| reject())?;
  Ok(Json(writer.seal(&bytes).map_err(|_| reject())?))
}

#[cfg(test)]
mod tests {
  use crypto::ClientHandshake;

  use super::*;
  async fn post(router: &Router, path: &str, body: &impl serde::Serialize) -> (u16, Vec<u8>) {
    let response = router
      .handle(
        Request::builder()
          .method("POST")
          .uri(path)
          .header("content-type", "application/json")
          .body(Body::from(serde_json::to_vec(body).unwrap()))
          .unwrap(),
      )
      .await;
    (
      response.status().as_u16(),
      to_bytes(response.into_body(), crypto::MAX_WIRE)
        .await
        .unwrap()
        .to_vec(),
    )
  }
  async fn prepare(router: &Router, path: &str, body: &str) -> (Envelope, crypto::ResponseReader) {
    let (client, hello) = ClientHandshake::begin().unwrap();
    let (status, bytes) = post(router, crypto::HANDSHAKE_PATH, &hello).await;
    assert_eq!(status, 200);
    let reply: ServerHello = serde_json::from_slice(&bytes).unwrap();
    client
      .finish(&reply, &reply.public_key)
      .unwrap()
      .seal(
        &serde_json::to_vec(&ApiRequest {
          method: if body.is_empty() { "GET" } else { "POST" }.into(),
          path: path.into(),
          body: body.into(),
          form: false,
        })
        .unwrap(),
      )
      .unwrap()
  }
  #[tokio::test]
  async fn encrypted_gateway_replay_tampering_and_plaintext_boundary() {
    let router = wrap(crate::router());
    for path in [
      "/api/counter",
      "/api/demos",
      "/api/studio",
      "/api/todos",
      "/api/echo",
      "/api/profile",
    ] {
      assert_eq!(
        router
          .handle(Request::builder().uri(path).body(Body::empty()).unwrap())
          .await
          .status()
          .as_u16(),
        403
      );
      assert_eq!(post(&router, path, &serde_json::json!({})).await.0, 403);
    }
    let (envelope, reader) = prepare(&router, "/api/counter", r#"{"action":"increment"}"#).await;
    let (a, b) = tokio::join!(
      post(&router, crypto::EXCHANGE_PATH, &envelope),
      post(&router, crypto::EXCHANGE_PATH, &envelope)
    );
    assert!(matches!((a.0, b.0), (200, 400) | (400, 200)));
    let bytes = if a.0 == 200 { a.1 } else { b.1 };
    let plain = reader
      .open(&serde_json::from_slice(&bytes).unwrap())
      .unwrap();
    let response: ApiResponse = serde_json::from_slice(&plain).unwrap();
    assert_eq!(response.status, 200);
    assert!(response.body.contains("\"value\":1"));
    let (mut bad, _) = prepare(&router, "/api/counter", r#"{"action":"increment"}"#).await;
    bad.ciphertext.push_str("00");
    assert_eq!(post(&router, crypto::EXCHANGE_PATH, &bad).await.0, 400);
    let (read, reader) = prepare(&router, "/api/counter", "").await;
    let (_, bytes) = post(&router, crypto::EXCHANGE_PATH, &read).await;
    let response: ApiResponse = serde_json::from_slice(
      &reader
        .open(&serde_json::from_slice(&bytes).unwrap())
        .unwrap(),
    )
    .unwrap();
    assert!(response.body.contains("\"revision\":1"));
    let (invalid, reader) = prepare(&router, "/api/profile", r#"{"username":"","age":20}"#).await;
    let (_, bytes) = post(&router, crypto::EXCHANGE_PATH, &invalid).await;
    let response: ApiResponse = serde_json::from_slice(
      &reader
        .open(&serde_json::from_slice(&bytes).unwrap())
        .unwrap(),
    )
    .unwrap();
    assert_eq!(response.status, 400);
  }
  #[tokio::test]
  async fn full_echo_history_fits_encrypted_response() {
    let business = crate::router();
    let value = serde_json::Value::String("\\".repeat(30_000));
    for _ in 0 .. 20 {
      assert_eq!(post(&business, "/api/echo", &value).await.0, 200);
    }
    let router = wrap(business);
    let (request, reader) = prepare(&router, "/api/demos", "").await;
    let (status, bytes) = post(&router, crypto::EXCHANGE_PATH, &request).await;
    assert_eq!(status, 200);
    let plain = reader
      .open(&serde_json::from_slice(&bytes).unwrap())
      .unwrap();
    let response: ApiResponse = serde_json::from_slice(&plain).unwrap();
    assert_eq!(response.status, 200);
    let state: topcoat_gpui_protocol::DemoSnapshot = serde_json::from_str(&response.body).unwrap();
    assert_eq!(state.echoes.len(), 20);
    assert_eq!(state.echoes[19], value);
  }

  #[tokio::test]
  async fn encrypted_suggestions_see_shared_writes_and_validate_input() {
    let router = wrap(crate::router());
    for (path, body, status) in [
      (
        "/api/todos",
        r#"{"action":"create","title":"学会双端补全"}"#,
        200,
      ),
      ("/api/suggestions", r#"{"kind":"todo","query":"学"}"#, 200),
      ("/api/suggestions", r#"{"kind":"bad","query":"学"}"#, 400),
      ("/api/suggestions", r#"{"kind":"todo","query":"a\nb"}"#, 400),
    ] {
      let (envelope, reader) = prepare(&router, path, body).await;
      let (outer, bytes) = post(&router, crypto::EXCHANGE_PATH, &envelope).await;
      assert_eq!(outer, 200);
      let plain = reader
        .open(&serde_json::from_slice(&bytes).unwrap())
        .unwrap();
      let reply: ApiResponse = serde_json::from_slice(&plain).unwrap();
      assert_eq!(reply.status, status);
      if path == "/api/suggestions" && status == 200 {
        let result: topcoat_gpui_protocol::Suggestions = serde_json::from_str(&reply.body).unwrap();
        assert_eq!(result.items[0].value, "学会双端补全");
        assert_eq!(
          result.items[0].source,
          topcoat_gpui_protocol::SuggestionSource::Shared
        );
      }
    }
    assert_eq!(
      post(
        &router,
        "/api/suggestions",
        &serde_json::json!({"kind":"todo","query":"学"})
      )
      .await
      .0,
      403
    );
    let body = serde_json::json!({"kind":"todo","query":"学".repeat(121)}).to_string();
    let (envelope, reader) = prepare(&router, "/api/suggestions", &body).await;
    let (_, bytes) = post(&router, crypto::EXCHANGE_PATH, &envelope).await;
    let plain = reader
      .open(&serde_json::from_slice(&bytes).unwrap())
      .unwrap();
    assert_eq!(
      serde_json::from_slice::<ApiResponse>(&plain)
        .unwrap()
        .status,
      400
    );
  }

  #[tokio::test]
  async fn every_handshake_has_a_fresh_server_key() {
    let router = wrap(crate::router());
    let (a, first) = ClientHandshake::begin().unwrap();
    let (b, second) = ClientHandshake::begin().unwrap();
    assert_ne!(first.public_key, second.public_key);
    let (status, bytes) = post(&router, crypto::HANDSHAKE_PATH, &first).await;
    assert_eq!(status, 200);
    let first: ServerHello = serde_json::from_slice(&bytes).unwrap();
    let (status, bytes) = post(&router, crypto::HANDSHAKE_PATH, &second).await;
    assert_eq!(status, 200);
    let second: ServerHello = serde_json::from_slice(&bytes).unwrap();
    assert_ne!(first.public_key, second.public_key);
    assert_ne!(first.session, second.session);
    assert!(a.finish(&first, &first.public_key).is_ok());
    assert!(b.finish(&second, &second.public_key).is_ok());
  }

  #[tokio::test]
  async fn rejects_expired_sessions_and_oversized_handshakes() {
    let identity = Identity::generate().unwrap();
    let pin = identity.public_key();
    let (client, hello) = ClientHandshake::begin().unwrap();
    let (reply, channel) = identity.accept(&hello).unwrap();
    let envelope = client.finish(&reply, &pin).unwrap().seal(b"{}").unwrap().0;
    let mut sessions = HashMap::new();
    sessions.insert(reply.session, (Instant::now() - TTL, channel));
    let router = Router::builder()
      .app_context(Gateway {
        sessions: Mutex::new(sessions),
        business: crate::router(),
      })
      .route(exchange)
      .route(handshake)
      .layer(PublicBoundary)
      .build();
    assert_eq!(post(&router, crypto::EXCHANGE_PATH, &envelope).await.0, 400);
    assert_eq!(
      post(&router, crypto::HANDSHAKE_PATH, &"x".repeat(9000))
        .await
        .0,
      400
    );
  }
}
