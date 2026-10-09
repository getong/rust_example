use std::time::Duration;

use gpui_topcoat_example::secure::SecureClient;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
};
use topcoat_gpui_protocol::{CounterSnapshot, DemoSnapshot, StudioSnapshot, crypto::*};

// Both ends share one thread: the encrypted HTTP client must yield to network I/O.
#[tokio::test(flavor = "current_thread")]
async fn encrypted_requests_yield_and_preserve_authenticated_errors() {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());
  let client = SecureClient::new(&base).unwrap();
  let server = tokio::spawn(async move {
    let mut sessions = std::collections::HashMap::new();
    for _ in 0 .. 8 {
      let (mut stream, _) = listener.accept().await.unwrap();
      let mut bytes = Vec::new();
      let (header_end, length) = loop {
        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer).await.unwrap();
        assert_ne!(n, 0);
        bytes.extend_from_slice(&buffer[.. n]);
        if let Some(end) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
          let headers = String::from_utf8_lossy(&bytes[.. end]);
          let length = headers
            .lines()
            .find_map(|line| {
              line
                .to_lowercase()
                .strip_prefix("content-length: ")
                .map(|n| n.parse::<usize>().unwrap())
            })
            .unwrap();
          break (end + 4, length);
        }
      };
      while bytes.len() < header_end + length {
        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer).await.unwrap();
        assert_ne!(n, 0);
        bytes.extend_from_slice(&buffer[.. n]);
      }
      let body = if bytes.starts_with(b"POST /pq/handshake ") {
        let hello = serde_json::from_slice(&bytes[header_end ..]).unwrap();
        let (reply, channel) = Identity::generate().unwrap().accept(&hello).unwrap();
        sessions.insert(reply.session.clone(), channel);
        serde_json::to_string(&reply).unwrap()
      } else {
        assert!(bytes.starts_with(b"POST /pq/exchange "));
        let envelope: Envelope = serde_json::from_slice(&bytes[header_end ..]).unwrap();
        let (plain, writer) = sessions
          .remove(&envelope.session)
          .unwrap()
          .open(&envelope)
          .unwrap();
        let input: ApiRequest = serde_json::from_slice(&plain).unwrap();
        let (status, body) = match input.path.as_str() {
          "/api/counter" => (200, r#"{"value":7,"revision":2}"#),
          "/api/demos" => (200, r#"{"todos":[],"echoes":[],"profiles":[]}"#),
          "/api/studio" => (200, r#"{"theme":"ocean","intensity":70,"revision":0}"#),
          _ => (400, "invalid profile"),
        };
        serde_json::to_string(
          &writer
            .seal(
              &serde_json::to_vec(&ApiResponse {
                status,
                body: body.into(),
              })
              .unwrap(),
            )
            .unwrap(),
        )
        .unwrap()
      };
      stream
        .write_all(
          format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: \
             {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
          )
          .as_bytes(),
        )
        .await
        .unwrap();
    }
  });
  let request = |path: &str| ApiRequest {
    method: "GET".into(),
    path: path.into(),
    body: String::new(),
    form: false,
  };
  tokio::time::timeout(Duration::from_secs(15), async {
    let (counter, demos, studio) = tokio::join!(
      client.request::<CounterSnapshot>(request("/api/counter")),
      client.request::<DemoSnapshot>(request("/api/demos")),
      client.request::<StudioSnapshot>(request("/api/studio"))
    );
    assert_eq!(counter.unwrap().value, 7);
    assert!(demos.unwrap().todos.is_empty());
    assert_eq!(studio.unwrap().intensity, 70);
    let error = client
      .request::<DemoSnapshot>(request("/api/profile"))
      .await
      .unwrap_err();
    assert!(error.contains("400") && error.contains("invalid profile"));
    server.await.unwrap();
    assert!(
      client
        .request::<CounterSnapshot>(request("/api/counter"))
        .await
        .is_err()
    );
  })
  .await
  .unwrap();
}
#[test]
fn requires_https_except_loopback_and_rejects_non_origin_urls() {
  assert!(SecureClient::new("http://localhost:3000").is_ok());
  assert!(SecureClient::new("https://example.com").is_ok());
  for url in [
    "ftp://localhost",
    "http://example.com",
    "http://127.0.0.1.example.com",
    "http://localhost/api",
    "http://user@localhost",
    "http://localhost/?q=1",
  ] {
    assert!(SecureClient::new(url).is_err());
  }
}
