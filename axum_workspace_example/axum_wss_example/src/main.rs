//! Example websocket server.
//!
//! Run the server with
//! ```not_rust
//! cargo run -p example-websockets --bin example-websockets
//! ```
//!
//! Run a browser client with
//! ```not_rust
//! firefox http://localhost:3000
//! ```
//!
//! Alternatively you can run the rust client (showing two
//! concurrent websocket connections being established) with
//! ```not_rust
//! cargo run -p example-websockets --bin example-client
//! ```

use std::{net::SocketAddr, ops::ControlFlow, path::PathBuf};

// allows to extract the IP of connecting user
use axum::{
  body::Bytes,
  extract::{
    connect_info::ConnectInfo,
    ws::{Message, WebSocket, WebSocketUpgrade},
  },
  response::IntoResponse,
  routing::any,
  Router,
};
use axum_extra::TypedHeader;
use axum_server::tls_rustls::RustlsConfig;
// allows to split the websocket stream into separate TX and RX branches
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use tower_http::{
  services::ServeDir,
  trace::{DefaultMakeSpan, TraceLayer},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Serialize)]
struct ServerEvent {
  event: &'static str,
  payload: Value,
}

#[tokio::main]
async fn main() {
  tracing_subscriber::registry()
    .with(
      tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| format!("{}=debug,tower_http=debug", env!("CARGO_CRATE_NAME")).into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();

  let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

  // build our application with some routes
  let app = Router::new()
    .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
    .route("/ws", any(ws_handler))
    // logging so we can see whats going on
    .layer(
      TraceLayer::new_for_http().make_span_with(DefaultMakeSpan::default().include_headers(true)),
    );

  let bind_addr = std::env::var("BIND_ADDR")
    .unwrap_or_else(|_| "127.0.0.1:3000".to_string())
    .parse::<SocketAddr>()
    .expect("BIND_ADDR must be a valid socket address, e.g. 0.0.0.0:3000");

  let tls_cert_path = std::env::var("TLS_CERT_PATH").ok();
  let tls_key_path = std::env::var("TLS_KEY_PATH").ok();

  match (tls_cert_path, tls_key_path) {
    (Some(cert_path), Some(key_path)) => {
      let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .expect("failed to load TLS cert/key from TLS_CERT_PATH and TLS_KEY_PATH");

      tracing::info!(
        "listening with TLS on https://{} (websocket endpoint: wss://{}/ws)",
        bind_addr,
        bind_addr
      );

      axum_server::bind_rustls(bind_addr, tls_config)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
    }
    (None, None) => {
      let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
      tracing::info!(
        "listening without TLS on http://{} (websocket endpoint: ws://{}/ws)",
        bind_addr,
        bind_addr
      );

      axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
      )
      .await
      .unwrap();
    }
    _ => {
      panic!("TLS_CERT_PATH and TLS_KEY_PATH must either both be set or both be unset");
    }
  }
}

/// The handler for the HTTP request (this gets called when the HTTP request lands at the start
/// of websocket negotiation). After this completes, the actual switching from HTTP to
/// websocket protocol will occur.
/// This is the last point where we can extract TCP/IP metadata such as IP address of the client
/// as well as things from HTTP headers such as user-agent of the browser etc.
async fn ws_handler(
  ws: WebSocketUpgrade,
  user_agent: Option<TypedHeader<headers::UserAgent>>,
  ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
  let user_agent = if let Some(TypedHeader(user_agent)) = user_agent {
    user_agent.to_string()
  } else {
    String::from("Unknown browser")
  };
  println!("`{user_agent}` at {addr} connected.");
  // finalize the upgrade process by returning upgrade callback.
  // we can customize the callback by sending additional info such as address.
  ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(mut socket: WebSocket, who: SocketAddr) {
  // send a ping (unsupported by some browsers) just to kick things off and get a response
  if socket
    .send(Message::Ping(Bytes::from_static(&[1, 2, 3])))
    .await
    .is_ok()
  {
    println!("Pinged {who}...");
  } else {
    println!("Could not send ping {who}!");
    // no Error here since the only thing we can do is to close the connection.
    // If we can not send messages, there is no way to salvage the statemachine anyway.
    return;
  }

  // receive single message from a client (we can either receive or send with socket).
  // this will likely be the Pong for our Ping or a hello message from client.
  // waiting for message from a client will block this task, but will not block other client's
  // connections.
  if let Some(msg) = socket.recv().await {
    if let Ok(msg) = msg {
      if process_message(msg, who).is_break() {
        return;
      }
    } else {
      println!("client {who} abruptly disconnected");
      return;
    }
  }

  // Since each client gets individual statemachine, we can pause handling
  // when necessary to wait for some external event (in this case illustrated by sleeping).
  // Waiting for this client to finish getting its greetings does not prevent other clients from
  // connecting to server and receiving their greetings.
  for i in 1 .. 5 {
    let greeting = ServerEvent {
      event: "greeting",
      payload: json!({ "times": i }),
    };

    if let Some(msg) = to_json_ws_message(&greeting) {
      if socket.send(msg).await.is_err() {
        println!("client {who} abruptly disconnected");
        return;
      }
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  }

  // By splitting socket we can send and receive at the same time. In this example we will send
  // unsolicited messages to client based on some sort of server's internal event (i.e .timer).
  let (mut sender, mut receiver) = socket.split();

  // Spawn a task that keeps pushing messages until the connection breaks or the peer closes.
  let mut send_task = tokio::spawn(async move {
    let mut i = 0_u64;
    loop {
      let tick_event = ServerEvent {
        event: "server_tick",
        payload: json!({ "index": i }),
      };

      // In case of any websocket error, we exit.
      if let Some(msg) = to_json_ws_message(&tick_event) {
        if sender.send(msg).await.is_err() {
          return i;
        }
      }

      i += 1;
      tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
  });

  // This second task will receive messages from client and print them on server console
  let mut recv_task = tokio::spawn(async move {
    let mut cnt = 0;
    while let Some(Ok(msg)) = receiver.next().await {
      cnt += 1;
      // print message and break if instructed to do so
      if process_message(msg, who).is_break() {
        break;
      }
    }
    cnt
  });

  // If any one of the tasks exit, abort the other.
  tokio::select! {
      rv_a = (&mut send_task) => {
          match rv_a {
              Ok(a) => println!("{a} messages sent to {who}"),
              Err(a) => println!("Error sending messages {a:?}")
          }
          recv_task.abort();
      },
      rv_b = (&mut recv_task) => {
          match rv_b {
              Ok(b) => println!("Received {b} messages"),
              Err(b) => println!("Error receiving messages {b:?}")
          }
          send_task.abort();
      }
  }

  // returning from the handler closes the websocket connection
  println!("Websocket context {who} destroyed");
}

fn to_json_ws_message<T: Serialize>(value: &T) -> Option<Message> {
  match serde_json::to_string(value) {
    Ok(serialized) => Some(Message::Text(serialized.into())),
    Err(err) => {
      tracing::error!("failed to serialize websocket json payload: {err}");
      None
    }
  }
}

/// helper to print contents of messages to stdout. Has special treatment for Close.
fn process_message(msg: Message, who: SocketAddr) -> ControlFlow<(), ()> {
  match msg {
    Message::Text(t) => {
      if let Ok(json_value) = serde_json::from_str::<Value>(&t) {
        println!(">>> {who} sent json text: {json_value}");
      } else {
        println!(">>> {who} sent str: {t:?}");
      }
    }
    Message::Binary(d) => {
      if let Ok(text) = std::str::from_utf8(&d) {
        if let Ok(json_value) = serde_json::from_str::<Value>(text) {
          println!(">>> {who} sent json binary: {json_value}");
        } else {
          println!(">>> {} sent utf8 binary: {:?}", who, text);
        }
      } else {
        println!(">>> {} sent {} bytes: {:?}", who, d.len(), d);
      }
    }
    Message::Close(c) => {
      if let Some(cf) = c {
        println!(
          ">>> {} sent close with code {} and reason `{}`",
          who, cf.code, cf.reason
        );
      } else {
        println!(">>> {who} somehow sent close message without CloseFrame");
      }
      return ControlFlow::Break(());
    }

    Message::Pong(v) => {
      println!(">>> {who} sent pong with {v:?}");
    }
    // You should never need to manually handle Message::Ping, as axum's websocket library
    // will do so for you automagically by replying with Pong and copying the v according to
    // spec. But if you need the contents of the pings you can see them here.
    Message::Ping(v) => {
      println!(">>> {who} sent ping with {v:?}");
    }
  }
  ControlFlow::Continue(())
}
