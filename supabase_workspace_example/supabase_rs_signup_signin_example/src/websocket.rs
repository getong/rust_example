use std::{net::SocketAddr, ops::ControlFlow};

use axum::{
  body::Bytes,
  extract::{
    ConnectInfo,
    ws::{Message, WebSocket, WebSocketUpgrade},
  },
  http::{HeaderMap, header},
  response::{Html, IntoResponse},
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
struct ServerEvent {
  event: &'static str,
  payload: Value,
}

pub async fn ws_demo_page() -> Html<&'static str> {
  Html(include_str!("../assets/ws-demo.html"))
}

pub async fn ws_handler(
  ws: WebSocketUpgrade,
  ConnectInfo(addr): ConnectInfo<SocketAddr>,
  headers: HeaderMap,
) -> impl IntoResponse {
  let user_agent = headers
    .get(header::USER_AGENT)
    .and_then(|value| value.to_str().ok())
    .unwrap_or("Unknown browser")
    .to_owned();

  tracing::info!("websocket connect user_agent={user_agent} addr={addr}");
  ws.on_upgrade(move |socket| handle_socket(socket, addr, user_agent))
}

async fn handle_socket(mut socket: WebSocket, who: SocketAddr, user_agent: String) {
  if socket
    .send(Message::Ping(Bytes::from_static(&[1, 2, 3])))
    .await
    .is_err()
  {
    tracing::warn!("failed to ping websocket client addr={who}");
    return;
  }

  let hello_event = ServerEvent {
    event: "connected",
    payload: json!({
      "message": "websocket connected",
      "client_addr": who,
      "user_agent": user_agent,
    }),
  };

  if let Some(msg) = to_json_ws_message(&hello_event) {
    if socket.send(msg).await.is_err() {
      tracing::warn!("failed to send hello event to addr={who}");
      return;
    }
  }

  if let Some(msg) = socket.recv().await {
    match msg {
      Ok(msg) => {
        if process_message(msg, who).is_break() {
          return;
        }
      }
      Err(err) => {
        tracing::warn!("client addr={who} disconnected during handshake: {err}");
        return;
      }
    }
  }

  for index in 1 ..= 3 {
    let greeting = ServerEvent {
      event: "greeting",
      payload: json!({ "times": index }),
    };

    if let Some(msg) = to_json_ws_message(&greeting) {
      if socket.send(msg).await.is_err() {
        tracing::warn!("client addr={who} disconnected during greeting");
        return;
      }
    }

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
  }

  let (mut sender, mut receiver) = socket.split();

  let mut send_task = tokio::spawn(async move {
    let mut index = 0usize;
    loop {
      let tick_event = ServerEvent {
        event: "server_tick",
        payload: json!({ "index": index }),
      };

      if let Some(msg) = to_json_ws_message(&tick_event) {
        if sender.send(msg).await.is_err() {
          return index;
        }
      }

      if index > 0 && index % 10 == 0 {
        if sender
          .send(Message::Ping(Bytes::from_static(b"keepalive")))
          .await
          .is_err()
        {
          return index;
        }
      }

      index += 1;
      tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
  });

  let mut recv_task = tokio::spawn(async move {
    let mut count = 0;
    while let Some(Ok(msg)) = receiver.next().await {
      count += 1;
      if process_message(msg, who).is_break() {
        break;
      }
    }
    count
  });

  tokio::select! {
    send_result = (&mut send_task) => {
      match send_result {
        Ok(count) => tracing::info!("sent {count} websocket messages to addr={who}"),
        Err(err) => tracing::warn!("websocket send task failed for addr={who}: {err}"),
      }
      recv_task.abort();
    }
    recv_result = (&mut recv_task) => {
      match recv_result {
        Ok(count) => tracing::info!("received {count} websocket messages from addr={who}"),
        Err(err) => tracing::warn!("websocket recv task failed for addr={who}: {err}"),
      }
      send_task.abort();
    }
  }

  tracing::info!("websocket context destroyed addr={who}");
}

fn to_json_ws_message<T: Serialize>(value: &T) -> Option<Message> {
  match serde_json::to_string(value) {
    Ok(serialized) => Some(Message::Text(serialized.into())),
    Err(err) => {
      tracing::error!("failed to serialize websocket payload: {err}");
      None
    }
  }
}

fn process_message(msg: Message, who: SocketAddr) -> ControlFlow<(), ()> {
  match msg {
    Message::Text(text) => {
      if let Ok(json_value) = serde_json::from_str::<Value>(&text) {
        tracing::info!("ws text json addr={who} payload={json_value}");
      } else {
        tracing::info!("ws text addr={who} payload={text}");
      }
    }
    Message::Binary(data) => {
      if let Ok(text) = std::str::from_utf8(&data) {
        if let Ok(json_value) = serde_json::from_str::<Value>(text) {
          tracing::info!("ws binary json addr={who} payload={json_value}");
        } else {
          tracing::info!("ws binary utf8 addr={who} payload={text}");
        }
      } else {
        tracing::info!("ws binary raw addr={who} bytes={}", data.len());
      }
    }
    Message::Close(close_frame) => {
      if let Some(frame) = close_frame {
        tracing::info!(
          "ws close addr={who} code={} reason={}",
          frame.code,
          frame.reason
        );
      } else {
        tracing::info!("ws close addr={who} without frame");
      }
      return ControlFlow::Break(());
    }
    Message::Pong(value) => {
      tracing::info!("ws pong addr={who} payload={value:?}");
    }
    Message::Ping(value) => {
      tracing::info!("ws ping addr={who} payload={value:?}");
    }
  }

  ControlFlow::Continue(())
}
