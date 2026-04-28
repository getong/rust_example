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
    State,
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
use tokio::sync::broadcast;
use tower_http::{
  services::ServeDir,
  trace::{DefaultMakeSpan, TraceLayer},
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
struct AppState {
  room_tx: broadcast::Sender<String>,
}

#[derive(Debug, Serialize)]
struct ServerEvent {
  event: &'static str,
  payload: Value,
}

#[derive(Debug)]
struct ChatMessage {
  user: String,
  msg: String,
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
  let (room_tx, _room_rx) = broadcast::channel::<String>(100);
  let app_state = AppState { room_tx };

  // build our application with some routes
  let app = Router::new()
    .fallback_service(ServeDir::new(assets_dir).append_index_html_on_directories(true))
    .route("/ws", any(ws_handler))
    .with_state(app_state)
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
  State(state): State<AppState>,
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
  ws.on_upgrade(move |socket| handle_socket(socket, addr, state.room_tx))
}

/// Actual websocket statemachine (one will be spawned per connection)
async fn handle_socket(mut socket: WebSocket, who: SocketAddr, room_tx: broadcast::Sender<String>) {
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
      if process_message(msg, who, &room_tx).is_break() {
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
  let mut room_rx = room_tx.subscribe();

  // Spawn a task that keeps pushing messages until the connection breaks or the peer closes.
  let mut send_task = tokio::spawn(async move {
    let mut i = 0_u64;
    loop {
      tokio::select! {
        room_message = room_rx.recv() => {
          match room_message {
            Ok(message) => {
              if sender.send(Message::Text(message.into())).await.is_err() {
                return i;
              }
            }
            Err(_) => return i,
          }
        }
        _ = tokio::time::sleep(std::time::Duration::from_millis(300)) => {
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
        }
      }
    }
  });

  // This second task will receive messages from client and print them on server console
  let mut recv_task = tokio::spawn(async move {
    let mut cnt = 0;
    while let Some(Ok(msg)) = receiver.next().await {
      cnt += 1;
      // print message and break if instructed to do so
      if process_message(msg, who, &room_tx).is_break() {
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
fn process_message(
  msg: Message,
  who: SocketAddr,
  room_tx: &broadcast::Sender<String>,
) -> ControlFlow<(), ()> {
  match msg {
    Message::Text(t) => {
      maybe_broadcast_chat_message(&t, who, room_tx);
      if let Ok(json_value) = serde_json::from_str::<Value>(&t) {
        println!(">>> {who} sent json text: {json_value}");
      } else {
        println!(">>> {who} sent str: {t:?}");
      }
    }
    Message::Binary(d) => {
      if let Ok(text) = std::str::from_utf8(&d) {
        maybe_broadcast_chat_message(text, who, room_tx);
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

fn maybe_broadcast_chat_message(
  raw: &str,
  who: SocketAddr,
  room_tx: &broadcast::Sender<String>,
) {
  let Some(chat_message) = parse_chat_message(raw, who) else {
    return;
  };

  let room_event = ServerEvent {
    event: "chat_message",
    payload: json!({
      "user": chat_message.user,
      "msg": chat_message.msg,
      "from": who.to_string(),
    }),
  };

  if let Ok(serialized) = serde_json::to_string(&room_event) {
    let _ = room_tx.send(serialized);
  }
}

fn parse_chat_message(raw: &str, who: SocketAddr) -> Option<ChatMessage> {
  let json_value = serde_json::from_str::<Value>(raw).ok()?;

  if let Some(payload) = json_value
    .get("event")
    .and_then(Value::as_str)
    .filter(|event| *event == "chat_message")
    .and_then(|_| json_value.get("payload"))
  {
    return chat_message_from_value(payload, who);
  }

  chat_message_from_value(&json_value, who)
}

fn chat_message_from_value(value: &Value, who: SocketAddr) -> Option<ChatMessage> {
  let msg = value.get("msg")?.as_str()?.trim();
  if msg.is_empty() {
    return None;
  }

  let user = value
    .get("user")
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|user| !user.is_empty())
    .unwrap_or("Anonymous");

  Some(ChatMessage {
    user: format!("{} @ {}", user, who.port()),
    msg: msg.to_string(),
  })
}
