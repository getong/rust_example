use std::{net::SocketAddr, ops::ControlFlow};

use axum::{
  body::Bytes,
  extract::{
    ConnectInfo, Query, State,
    ws::{Message, WebSocket, WebSocketUpgrade},
  },
  http::{HeaderMap, header},
  response::{Html, IntoResponse, Response},
};
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use supabase_auth::error::Error as SupabaseAuthError;
use tokio::sync::mpsc;

use crate::{
  auth::map_supabase_auth_error,
  error::{AppError, AppResult},
  models::AppState,
};

#[derive(Debug, Serialize)]
struct ServerEvent {
  event: &'static str,
  payload: Value,
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct WsAuthQuery {
  access_token: Option<String>,
  token: Option<String>,
}

#[derive(Debug, Clone)]
struct SocketUser {
  id: String,
  email: String,
}

pub async fn ws_demo_page() -> Html<&'static str> {
  Html(include_str!("../assets/ws-demo.html"))
}

pub async fn ws_handler(
  State(state): State<AppState>,
  ws: WebSocketUpgrade,
  ConnectInfo(addr): ConnectInfo<SocketAddr>,
  Query(query): Query<WsAuthQuery>,
  headers: HeaderMap,
) -> AppResult<Response> {
  let user_agent = headers
    .get(header::USER_AGENT)
    .and_then(|value| value.to_str().ok())
    .unwrap_or("Unknown browser")
    .to_owned();
  let access_token = extract_access_token(&headers, &query)?;
  let auth_client = state.supabase_auth_client.as_ref().ok_or_else(|| {
    AppError::service_unavailable(
      "supabase_auth is not configured; set SUPABASE_JWT_SECRET to enable websocket auth",
    )
  })?;
  let user = auth_client
    .get_user(&access_token)
    .await
    .map_err(map_websocket_auth_error)?;
  let socket_user = SocketUser {
    id: user.id.to_string(),
    email: user.email,
  };

  tracing::info!(
    "websocket authenticated user_id={} email={} user_agent={} addr={addr}",
    socket_user.id,
    socket_user.email,
    user_agent
  );

  Ok(
    ws.on_upgrade(move |socket| handle_socket(socket, addr, user_agent, socket_user))
      .into_response(),
  )
}

async fn handle_socket(
  mut socket: WebSocket,
  who: SocketAddr,
  user_agent: String,
  user: SocketUser,
) {
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
      "user": {
        "id": user.id,
        "email": user.email,
      },
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
        let (control_flow, reply) = process_message(msg, who);
        if let Some(reply) = reply {
          if socket.send(reply).await.is_err() {
            tracing::warn!("failed to send websocket echo during handshake addr={who}");
            return;
          }
        }

        if control_flow.is_break() {
          return;
        }
      }
      Err(err) => {
        tracing::warn!("client addr={who} disconnected during handshake: {err}");
        return;
      }
    }
  }

  for index in 1..=3 {
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
  let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<Message>();

  let mut send_task = tokio::spawn(async move {
    let mut index = 0usize;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    loop {
      tokio::select! {
        maybe_msg = outbound_rx.recv() => {
          match maybe_msg {
            Some(msg) => {
              if sender.send(msg).await.is_err() {
                return index;
              }
            }
            None => return index,
          }
        }
        _ = interval.tick() => {
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
        }
      }
    }
  });

  let mut recv_task = tokio::spawn(async move {
    let mut count = 0;
    while let Some(Ok(msg)) = receiver.next().await {
      count += 1;
      let (control_flow, reply) = process_message(msg, who);
      if let Some(reply) = reply {
        if outbound_tx.send(reply).is_err() {
          break;
        }
      }

      if control_flow.is_break() {
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

fn extract_access_token(headers: &HeaderMap, query: &WsAuthQuery) -> AppResult<String> {
  bearer_token_from_header(headers)
    .or_else(|| sanitize_token(query.access_token.as_deref()))
    .or_else(|| sanitize_token(query.token.as_deref()))
    .ok_or_else(|| {
      AppError::unauthorized(
        "missing supabase access token; use Authorization: Bearer <token> or ?access_token=<token>",
      )
    })
}

fn bearer_token_from_header(headers: &HeaderMap) -> Option<String> {
  let header_value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
  let trimmed = header_value.trim();

  trimmed
    .strip_prefix("Bearer ")
    .or_else(|| trimmed.strip_prefix("bearer "))
    .and_then(|token| sanitize_token(Some(token)))
}

fn sanitize_token(token: Option<&str>) -> Option<String> {
  token
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(ToOwned::to_owned)
}

fn map_websocket_auth_error(err: SupabaseAuthError) -> AppError {
  match err {
    SupabaseAuthError::WrongToken | SupabaseAuthError::NotAuthenticated => {
      AppError::unauthorized("invalid or expired supabase access token")
    }
    SupabaseAuthError::AuthError { status, .. } if status.as_u16() == 401 => {
      AppError::unauthorized("invalid or expired supabase access token")
    }
    other => map_supabase_auth_error(other),
  }
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

fn process_message(msg: Message, who: SocketAddr) -> (ControlFlow<(), ()>, Option<Message>) {
  match msg {
    Message::Text(text) => {
      let text = text.to_string();
      let payload = if let Ok(json_value) = serde_json::from_str::<Value>(&text) {
        tracing::info!("ws text json addr={who} payload={json_value}");
        json!({
          "message_type": "text_json",
          "body": json_value,
        })
      } else {
        tracing::info!("ws text addr={who} payload={text}");
        json!({
          "message_type": "text",
          "body": text,
        })
      };

      return (ControlFlow::Continue(()), echo_message(payload));
    }
    Message::Binary(data) => {
      let payload = if let Ok(text) = std::str::from_utf8(&data) {
        if let Ok(json_value) = serde_json::from_str::<Value>(text) {
          tracing::info!("ws binary json addr={who} payload={json_value}");
          json!({
            "message_type": "binary_json",
            "body": json_value,
            "byte_len": data.len(),
          })
        } else {
          tracing::info!("ws binary utf8 addr={who} payload={text}");
          json!({
            "message_type": "binary_utf8",
            "body": text,
            "byte_len": data.len(),
          })
        }
      } else {
        tracing::info!("ws binary raw addr={who} bytes={}", data.len());
        json!({
          "message_type": "binary_raw",
          "body": format!("<{} binary bytes>", data.len()),
          "byte_len": data.len(),
        })
      };

      return (ControlFlow::Continue(()), echo_message(payload));
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
      return (ControlFlow::Break(()), None);
    }
    Message::Pong(value) => {
      tracing::info!("ws pong addr={who} payload={value:?}");
    }
    Message::Ping(value) => {
      tracing::info!("ws ping addr={who} payload={value:?}");
    }
  }

  (ControlFlow::Continue(()), None)
}

fn echo_message(payload: Value) -> Option<Message> {
  let event = ServerEvent {
    event: "client_message",
    payload,
  };

  to_json_ws_message(&event)
}
