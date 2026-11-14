use std::{
  io::{BufRead, BufReader, Read, Write},
  net::{TcpListener, TcpStream},
  sync::mpsc,
  thread,
  time::Duration,
};

use gpui_mai_ui_client::{
  generation::{self, Event},
  session::Session,
};
use serde_json::{Value, json};

const PAGE: &str =
  "<!doctype html><html><head><style>body{color:blue}</style></head><body>完成</body></html>";

fn read_request(stream: &mut TcpStream) -> (Vec<String>, Value) {
  stream
    .set_read_timeout(Some(Duration::from_secs(5)))
    .unwrap();
  let mut reader = BufReader::new(stream);
  let mut headers = Vec::new();
  let mut content_length = 0;
  loop {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    if line == "\r\n" {
      break;
    }
    if let Some(length) = line.to_ascii_lowercase().strip_prefix("content-length:") {
      content_length = length.trim().parse().unwrap();
    }
    headers.push(line.trim().to_owned());
  }
  let mut body = vec![0; content_length];
  reader.read_exact(&mut body).unwrap();
  (headers, serde_json::from_slice(&body).unwrap())
}

#[test]
fn transport_streams_with_authentication_and_preserves_usage() {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let endpoint = format!(
    "http://{}/v1/chat/completions",
    listener.local_addr().unwrap()
  );
  let server = thread::spawn(move || {
    let (mut stream, _) = listener.accept().unwrap();
    let (headers, request) = read_request(&mut stream);
    assert!(
      headers
        .iter()
        .any(|header| header == "authorization: Bearer test-key")
    );
    assert_eq!(request["stream"], true);
    assert_eq!(request["messages"][1]["content"], "设计一个设置页面");
    write!(
      stream,
      "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let stream_body = format!(
      "data: {}\n\ndata: {}\n\ndata: {}\n\ndata: [DONE]\n\n",
      json!({"choices": [{"delta": {"content": PAGE}}]}),
      json!({"choices": [{"delta": {}, "finish_reason": "stop"}]}),
      json!({"choices": [], "usage": {"completion_tokens": 85}})
    );
    for chunk in stream_body.as_bytes().chunks(17) {
      stream.write_all(chunk).unwrap();
    }
  });
  let request = Session::default().request("设计一个设置页面").unwrap();
  let job = generation::start(endpoint, request, Some("test-key".into())).unwrap();
  let mut received = String::new();
  loop {
    match job.events.recv_timeout(Duration::from_secs(10)).unwrap() {
      Event::Connected => {}
      Event::Delta { content, .. } => received.push_str(&content),
      Event::Finished(result) => {
        let output = result.unwrap();
        assert_eq!(output.content, PAGE);
        assert_eq!(output.tokens, Some(85));
        break;
      }
    }
  }
  assert_eq!(received, PAGE);
  server.join().unwrap();
}

#[test]
fn cancellation_interrupts_a_stalled_response_without_waiting_for_timeout() {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let endpoint = format!(
    "http://{}/v1/chat/completions",
    listener.local_addr().unwrap()
  );
  let (closed, receiver) = mpsc::channel();
  let server = thread::spawn(move || {
    let (mut stream, _) = listener.accept().unwrap();
    read_request(&mut stream);
    write!(
      stream,
      "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    stream.flush().unwrap();
    let mut buffer = [0; 1];
    let disconnected = matches!(stream.read(&mut buffer), Ok(0));
    let _ = closed.send(disconnected);
  });
  let job = generation::start(endpoint, Session::default().request("设计").unwrap(), None).unwrap();
  assert!(matches!(
    job.events.recv_timeout(Duration::from_secs(10)).unwrap(),
    Event::Connected
  ));
  drop(job);
  assert!(receiver.recv_timeout(Duration::from_secs(3)).unwrap());
  server.join().unwrap();
}

#[test]
fn http_errors_reach_the_ui_with_service_details() {
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let endpoint = format!(
    "http://{}/v1/chat/completions",
    listener.local_addr().unwrap()
  );
  let server = thread::spawn(move || {
    let (mut stream, _) = listener.accept().unwrap();
    read_request(&mut stream);
    write!(
      stream,
      "HTTP/1.1 404 Not Found\r\nContent-Length: 13\r\nConnection: close\r\n\r\nmodel missing"
    )
    .unwrap();
  });
  let job = generation::start(endpoint, Session::default().request("设计").unwrap(), None).unwrap();
  let Event::Finished(result) = job.events.recv_timeout(Duration::from_secs(10)).unwrap() else {
    panic!("HTTP failure must not report a successful connection");
  };
  let error = result.err().unwrap().to_string();
  assert!(error.contains("404"));
  assert!(error.contains("model missing"));
  server.join().unwrap();
}

#[test]
fn grounding_transport_sends_screenshot_and_accepts_reference_response() {
  use gpui_mai_ui_client::{generation::ResponseFormat, grounding::Screenshot, session::Settings};
  let listener = TcpListener::bind("127.0.0.1:0").unwrap();
  let endpoint = format!(
    "http://{}/v1/chat/completions",
    listener.local_addr().unwrap()
  );
  let server = thread::spawn(move || {
    let (mut stream, _) = listener.accept().unwrap();
    let (_, request) = read_request(&mut stream);
    assert_eq!(request["messages"][1]["content"][0]["text"], "定位设置");
    assert!(
      request["messages"][1]["content"][1]["image_url"]["url"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,")
    );
    let response = json!({"choices":[{"delta":{"content":"<answer>{\"coordinate\":[499,200]}</answer>"},"finish_reason":"stop"}],"usage":{"completion_tokens":12}});
    write!(
      stream,
      "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: \
       {response}\n\ndata: [DONE]\n\n"
    )
    .unwrap();
  });
  let mut bytes = std::io::Cursor::new(Vec::new());
  image::DynamicImage::new_rgb8(10, 10)
    .write_to(&mut bytes, image::ImageFormat::Png)
    .unwrap();
  let screenshot = Screenshot::from_bytes("test.png".into(), bytes.get_ref()).unwrap();
  let request = screenshot
    .request(&Settings::default(), "定位设置")
    .unwrap();
  let job =
    generation::start_with_format(endpoint, request, None, ResponseFormat::Grounding).unwrap();
  loop {
    if let Event::Finished(result) = job.events.recv_timeout(Duration::from_secs(10)).unwrap() {
      let output = result.unwrap();
      assert_eq!(
        output.content,
        "<answer>{\"coordinate\":[499,200]}</answer>"
      );
      assert_eq!(output.tokens, Some(12));
      break;
    }
  }
  server.join().unwrap();
}
