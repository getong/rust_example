use gpui_topcoat_example::axum_api::health;
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
};

#[tokio::test(flavor = "current_thread")]
async fn calls_axum_path_and_rejects_failed_or_invalid_health() {
  for (status, body, expected) in [
    (200, r#"{"status":"ok"}"#.to_string(), true),
    (503, r#"{"status":"ok"}"#.to_string(), false),
    (302, r#"{"status":"ok"}"#.to_string(), false),
    (200, r#"{"status":"down"}"#.to_string(), false),
    (200, "not json".to_string(), false),
    (200, "x".repeat(1025), false),
  ] {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
      let (mut stream, _) = listener.accept().await.unwrap();
      let mut request = Vec::new();
      while !request.windows(4).any(|w| w == b"\r\n\r\n") {
        let mut buf = [0; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        assert_ne!(n, 0);
        request.extend_from_slice(&buf[..n]);
      }
      assert!(request.starts_with(b"GET /healthz HTTP/1.1\r\n"));
      stream.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await.unwrap();
    });
    assert_eq!(health(&base).await.is_ok(), expected);
    server.await.unwrap();
    assert!(health(&base).await.is_err());
  }
}

#[tokio::test]
async fn health_uses_the_same_origin_policy_as_encrypted_requests() {
  for base in [
    "http://example.com",
    "http://localhost/api",
    "http://user@localhost",
    "ftp://localhost",
  ] {
    assert!(health(base).await.is_err());
  }
}
