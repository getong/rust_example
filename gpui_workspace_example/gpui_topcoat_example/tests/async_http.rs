use std::time::Duration;

use gpui_topcoat_example::{DemoCommand, demo_request, request, studio_request};
use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
};

// The server shares a single-thread runtime with the clients: blocking HTTP would
// prevent it from replying. No external backend or fixed port is required.
#[tokio::test(flavor = "current_thread")]
async fn requests_yield_to_io_and_preserve_response_errors() {
  let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
  let base = format!("http://{}", listener.local_addr().unwrap());
  let server = tokio::spawn(async move {
    for _ in 0 .. 4 {
      let (mut stream, _) = listener.accept().await.unwrap();
      let mut bytes = Vec::new();
      loop {
        let mut buffer = [0; 1024];
        let size = stream.read(&mut buffer).await.unwrap();
        assert_ne!(size, 0);
        bytes.extend_from_slice(&buffer[.. size]);
        if bytes.windows(4).any(|part| part == b"\r\n\r\n") {
          break;
        }
      }
      let headers = String::from_utf8_lossy(&bytes);
      let (status, body) = if headers.starts_with("GET /api/counter ") {
        ("200 OK", r#"{"value":7,"revision":2}"#)
      } else if headers.starts_with("GET /api/demos ") {
        ("200 OK", r#"{"todos":[],"echoes":[],"profiles":[]}"#)
      } else if headers.starts_with("GET /api/studio ") {
        ("200 OK", r#"{"theme":"ocean","intensity":70,"revision":0}"#)
      } else {
        ("400 Bad Request", "invalid profile")
      };
      let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: \
         close\r\n\r\n{body}",
        body.len()
      );
      stream.write_all(response.as_bytes()).await.unwrap();
    }
  });
  tokio::time::timeout(Duration::from_secs(10), async {
    let (counter, demos, studio) = tokio::join!(
      request(&base, None),
      demo_request(&base, DemoCommand::Refresh),
      studio_request(&base, None),
    );
    assert_eq!(counter.unwrap().value, 7);
    assert!(demos.unwrap().todos.is_empty());
    assert_eq!(studio.unwrap().intensity, 70);
    let error = demo_request(
      &base,
      DemoCommand::Profile(topcoat_gpui_protocol::Profile {
        username: String::new(),
        age: 20,
      }),
    )
    .await
    .unwrap_err();
    assert!(error.contains("400") && error.contains("invalid profile"));
    server.await.unwrap();
    assert!(request(&base, None).await.is_err());
  })
  .await
  .expect("asynchronous HTTP must allow the server to make progress");
}
