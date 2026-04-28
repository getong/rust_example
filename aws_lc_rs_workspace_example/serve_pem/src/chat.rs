use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use axum::extract::ws::Message;
use futures_util::{sink::SinkExt, stream::StreamExt};
use tokio::sync::{Mutex, broadcast};

use crate::{
  AppState, MAX_CHAT_MESSAGE_BYTES, MAX_CHAT_ROOM_NAME_BYTES, MAX_CHAT_USER_NAME_BYTES,
  models::{ClientWsEnvelope, ServerWsEnvelope},
};

const CHAT_ROOM_CHANNEL_CAPACITY: usize = 256;

pub(crate) struct ChatState {
  rooms: Mutex<HashMap<String, ChatRoom>>,
}

struct ChatRoom {
  sender: broadcast::Sender<String>,
  members: usize,
}

pub(crate) struct JoinedRoom {
  pub(crate) room: String,
  pub(crate) sender: broadcast::Sender<String>,
  pub(crate) member_count: usize,
}

#[derive(Debug)]
struct ParsedChatMessage {
  user: String,
  msg: String,
}

impl ChatState {
  pub(crate) fn new() -> Self {
    Self {
      rooms: Mutex::new(HashMap::new()),
    }
  }

  pub(crate) async fn join_room(&self, room: &str) -> JoinedRoom {
    let mut rooms = self.rooms.lock().await;
    let room_state = rooms.entry(room.to_owned()).or_insert_with(|| {
      let (sender, _receiver) = broadcast::channel(CHAT_ROOM_CHANNEL_CAPACITY);
      ChatRoom { sender, members: 0 }
    });
    room_state.members += 1;

    JoinedRoom {
      room: room.to_owned(),
      sender: room_state.sender.clone(),
      member_count: room_state.members,
    }
  }

  pub(crate) async fn leave_room(&self, room: &str) -> usize {
    let mut rooms = self.rooms.lock().await;
    let Some(room_state) = rooms.get_mut(room) else {
      return 0;
    };

    room_state.members = room_state.members.saturating_sub(1);
    let member_count = room_state.members;
    if member_count == 0 {
      rooms.remove(room);
    }
    member_count
  }
}

pub(crate) fn normalize_room_name(room: &str) -> Option<String> {
  let trimmed = room.trim();
  if trimmed.is_empty() || trimmed.len() > MAX_CHAT_ROOM_NAME_BYTES {
    return None;
  }

  let normalized = trimmed
    .chars()
    .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    .collect::<String>();

  if normalized.is_empty() {
    return None;
  }

  Some(normalized)
}

pub(crate) async fn run_socket(
  state: Arc<AppState>,
  room: String,
  who: SocketAddr,
  socket: axum::extract::ws::WebSocket,
) {
  let joined_room = state.chat.join_room(&room).await;
  let room_sender = joined_room.sender.clone();
  let joined_event = ServerWsEnvelope::presence(
    "joined",
    &joined_room.room,
    joined_room.member_count,
    Some(who.to_string()),
  );
  let _ = broadcast_room_event(&room_sender, &joined_event);

  let (mut sender, mut receiver) = socket.split();
  let mut room_receiver = room_sender.subscribe();

  let welcome_event = ServerWsEnvelope::welcome(&joined_room.room, who, joined_room.member_count);
  if send_event(&mut sender, &welcome_event).await.is_err() {
    let member_count = state.chat.leave_room(&joined_room.room).await;
    let left_event = ServerWsEnvelope::presence(
      "left",
      &joined_room.room,
      member_count,
      Some(who.to_string()),
    );
    let _ = broadcast_room_event(&room_sender, &left_event);
    return;
  }

  let mut send_task = tokio::spawn(async move {
    while let Ok(message) = room_receiver.recv().await {
      if sender.send(Message::Text(message.into())).await.is_err() {
        break;
      }
    }
  });

  let recv_state = state.clone();
  let recv_room = joined_room.room.clone();
  let recv_sender = room_sender.clone();
  let mut recv_task = tokio::spawn(async move {
    while let Some(result) = receiver.next().await {
      let Ok(message) = result else {
        break;
      };

      match message {
        Message::Text(text) => {
          process_incoming_message(&recv_sender, &recv_room, who, &text);
        }
        Message::Binary(bytes) => {
          if let Ok(text) = std::str::from_utf8(&bytes) {
            process_incoming_message(&recv_sender, &recv_room, who, text);
          } else {
            let _ = broadcast_room_event(
              &recv_sender,
              &ServerWsEnvelope::error("binary websocket messages must contain UTF-8 JSON"),
            );
          }
        }
        Message::Close(_) => break,
        Message::Ping(_) | Message::Pong(_) => {}
      }
    }

    let member_count = recv_state.chat.leave_room(&recv_room).await;
    let left_event =
      ServerWsEnvelope::presence("left", &recv_room, member_count, Some(who.to_string()));
    let _ = broadcast_room_event(&recv_sender, &left_event);
  });

  tokio::select! {
    _ = &mut send_task => recv_task.abort(),
    _ = &mut recv_task => send_task.abort(),
  }
}

async fn send_event(
  sender: &mut futures_util::stream::SplitSink<axum::extract::ws::WebSocket, Message>,
  event: &ServerWsEnvelope,
) -> Result<(), ()> {
  let serialized = serde_json::to_string(event).map_err(|_| ())?;
  sender
    .send(Message::Text(serialized.into()))
    .await
    .map_err(|_| ())
}

fn process_incoming_message(
  room_sender: &broadcast::Sender<String>,
  room: &str,
  who: SocketAddr,
  raw: &str,
) {
  let Some(chat_message) = parse_chat_message(raw, who) else {
    return;
  };

  let event = ServerWsEnvelope::chat_message(room, who, &chat_message.user, &chat_message.msg);
  let _ = broadcast_room_event(room_sender, &event);
}

fn parse_chat_message(raw: &str, who: SocketAddr) -> Option<ParsedChatMessage> {
  let envelope = serde_json::from_str::<ClientWsEnvelope>(raw).ok()?;
  if envelope.event != "chat_message" {
    return None;
  }

  let payload = envelope.payload;
  let msg = payload.msg.trim();
  if msg.is_empty() || msg.len() > MAX_CHAT_MESSAGE_BYTES {
    return None;
  }

  let user = payload
    .user
    .as_deref()
    .map(str::trim)
    .filter(|user| !user.is_empty() && user.len() <= MAX_CHAT_USER_NAME_BYTES)
    .unwrap_or("Anonymous");

  Some(ParsedChatMessage {
    user: format!("{user} @ {}", who.port()),
    msg: msg.to_owned(),
  })
}

fn broadcast_room_event(
  room_sender: &broadcast::Sender<String>,
  event: &ServerWsEnvelope,
) -> Result<usize, serde_json::Error> {
  let serialized = serde_json::to_string(event)?;
  Ok(room_sender.send(serialized).unwrap_or_default())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn normalize_room_name_accepts_safe_tokens() {
    assert_eq!(
      normalize_room_name("general-room_1"),
      Some("general-room_1".to_owned())
    );
  }

  #[test]
  fn normalize_room_name_rejects_empty_or_invalid_input() {
    assert_eq!(normalize_room_name("   "), None);
    assert_eq!(normalize_room_name("!!!"), None);
  }

  #[test]
  fn parse_chat_message_accepts_valid_payload() {
    let message = parse_chat_message(
      r#"{"event":"chat_message","payload":{"user":"alice","msg":"hello"}}"#,
      "127.0.0.1:3030".parse().unwrap(),
    )
    .unwrap();

    assert!(message.user.starts_with("alice @ "));
    assert_eq!(message.msg, "hello");
  }

  #[test]
  fn parse_chat_message_rejects_empty_message() {
    assert!(
      parse_chat_message(
        r#"{"event":"chat_message","payload":{"user":"alice","msg":"   "}}"#,
        "127.0.0.1:3030".parse().unwrap(),
      )
      .is_none()
    );
  }
}
