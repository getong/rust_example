use std::{
  env,
  net::SocketAddr,
  thread,
  time::{SystemTime, UNIX_EPOCH},
};

use bevy::prelude::*;
use redis::AsyncCommands;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::agones::AgonesGameServerInfo;

const DEFAULT_KEY_PREFIX: &str = "player";
const DEFAULT_TTL_SECONDS: u64 = 6 * 60 * 60;
const DEFAULT_NAMESPACE: &str = "default";
const DEFAULT_LOCAL_HOST: &str = "127.0.0.1";

pub(crate) struct PlayerServerRegistryPlugin;

impl Plugin for PlayerServerRegistryPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<PlayerServerRegistry>()
      .add_systems(Startup, start_player_server_registry);
  }
}

#[derive(Resource, Default)]
pub(crate) struct PlayerServerRegistry {
  sender: Option<mpsc::UnboundedSender<PlayerServerCommand>>,
}

impl PlayerServerRegistry {
  pub(crate) fn upsert(&self, record: PlayerServerRecord) {
    let Some(sender) = self.sender.as_ref() else {
      return;
    };

    if let Err(err) = sender.send(PlayerServerCommand::Upsert { record }) {
      eprintln!("game_server redis player registry upsert queue closed: {err}");
    }
  }

  pub(crate) fn remove(&self, player_id: u64) {
    let Some(sender) = self.sender.as_ref() else {
      return;
    };

    if let Err(err) = sender.send(PlayerServerCommand::Remove {
      player_id: player_id.to_string(),
    }) {
      eprintln!("game_server redis player registry remove queue closed: {err}");
    }
  }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PlayerServerRecord {
  pub(crate) player_id: String,
  pub(crate) gs_name: String,
  pub(crate) namespace: String,
  pub(crate) ip: String,
  pub(crate) port: u16,
  pub(crate) server_addr: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) room: Option<String>,
  pub(crate) updated_at_ms: u64,
}

#[derive(Clone, Debug)]
struct PlayerServerRegistryConfig {
  redis_url: String,
  key_prefix: String,
  ttl_seconds: u64,
}

#[derive(Debug)]
enum PlayerServerCommand {
  Upsert { record: PlayerServerRecord },
  Remove { player_id: String },
}

pub(crate) fn player_server_record(
  player_id: u64,
  room: Option<&str>,
  agones_info: &AgonesGameServerInfo,
  bind_addr: SocketAddr,
) -> PlayerServerRecord {
  let endpoint = server_endpoint(agones_info, bind_addr);
  PlayerServerRecord {
    player_id: player_id.to_string(),
    gs_name: endpoint.gs_name,
    namespace: endpoint.namespace,
    ip: endpoint.ip.clone(),
    port: endpoint.port,
    server_addr: format!("{}:{}", endpoint.ip, endpoint.port),
    room: room
      .map(str::trim)
      .filter(|room| !room.is_empty())
      .map(ToOwned::to_owned),
    updated_at_ms: unix_time_ms(),
  }
}

fn start_player_server_registry(mut registry: ResMut<PlayerServerRegistry>) {
  let Some(config) = PlayerServerRegistryConfig::from_env() else {
    println!("game_server redis player registry disabled");
    return;
  };

  registry.sender = spawn_player_server_registry(config);
}

fn spawn_player_server_registry(
  config: PlayerServerRegistryConfig,
) -> Option<mpsc::UnboundedSender<PlayerServerCommand>> {
  let (sender, receiver) = mpsc::unbounded_channel();

  match thread::Builder::new()
    .name("player-server-redis".to_string())
    .spawn(move || {
      let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
      {
        Ok(runtime) => runtime,
        Err(err) => {
          eprintln!("game_server redis player registry runtime error: {err}");
          return;
        }
      };

      runtime.block_on(run_player_server_registry(config, receiver));
    }) {
    Ok(_thread) => Some(sender),
    Err(err) => {
      eprintln!("game_server redis player registry thread start error: {err}");
      None
    }
  }
}

async fn run_player_server_registry(
  config: PlayerServerRegistryConfig,
  mut receiver: mpsc::UnboundedReceiver<PlayerServerCommand>,
) {
  println!(
    "game_server redis player registry enabled, ttl={}s",
    config.ttl_seconds
  );

  let client = match redis::Client::open(config.redis_url.as_str()) {
    Ok(client) => client,
    Err(err) => {
      eprintln!("game_server redis player registry invalid url: {err}");
      return;
    }
  };
  let mut connection = match client.get_connection_manager().await {
    Ok(connection) => connection,
    Err(err) => {
      eprintln!("game_server redis player registry connect error: {err}");
      return;
    }
  };

  while let Some(command) = receiver.recv().await {
    match command {
      PlayerServerCommand::Upsert { record } => {
        let key = player_server_key(&config.key_prefix, record.player_id.as_str());
        let value = match serde_json::to_string(&record) {
          Ok(value) => value,
          Err(err) => {
            eprintln!("game_server redis player registry encode error: {err}");
            continue;
          }
        };
        let result: redis::RedisResult<()> = connection
          .set_ex(key.as_str(), value, config.ttl_seconds)
          .await;
        if let Err(err) = result {
          eprintln!("game_server redis player registry SETEX {key} error: {err}");
        }
      }
      PlayerServerCommand::Remove { player_id } => {
        let key = player_server_key(&config.key_prefix, player_id.as_str());
        let result: redis::RedisResult<usize> = connection.del(key.as_str()).await;
        if let Err(err) = result {
          eprintln!("game_server redis player registry DEL {key} error: {err}");
        }
      }
    }
  }
}

impl PlayerServerRegistryConfig {
  fn from_env() -> Option<Self> {
    let enabled = env_bool("PLAYER_SERVER_REGISTRY_ENABLED").unwrap_or(true);
    if !enabled {
      return None;
    }

    let redis_url = first_env(&["PLAYER_SERVER_REDIS_URL", "REDIS_URL"])?;
    let key_prefix =
      first_env(&["PLAYER_SERVER_REDIS_KEY_PREFIX"]).unwrap_or_else(|| DEFAULT_KEY_PREFIX.into());
    let ttl_seconds = env_u64("PLAYER_SERVER_REDIS_TTL_SECONDS")
      .filter(|ttl| *ttl > 0)
      .unwrap_or(DEFAULT_TTL_SECONDS);

    Some(Self {
      redis_url,
      key_prefix,
      ttl_seconds,
    })
  }
}

struct ServerEndpoint {
  gs_name: String,
  namespace: String,
  ip: String,
  port: u16,
}

fn server_endpoint(agones_info: &AgonesGameServerInfo, bind_addr: SocketAddr) -> ServerEndpoint {
  let gs_name = first_env(&["PLAYER_SERVER_GAME_SERVER_NAME", "GAME_SERVER_NAME"])
    .or_else(|| agones_info.name.clone())
    .or_else(|| first_env(&["HOSTNAME"]))
    .unwrap_or_else(|| "local-game-server".to_string());
  let namespace = first_env(&["PLAYER_SERVER_NAMESPACE"])
    .or_else(|| agones_info.namespace.clone())
    .unwrap_or_else(|| DEFAULT_NAMESPACE.to_string());
  let ip = first_env(&["PLAYER_SERVER_PUBLIC_HOST", "GAME_SERVER_PUBLIC_HOST"])
    .or_else(|| agones_info.address.clone())
    .unwrap_or_else(|| {
      if bind_addr.ip().is_unspecified() {
        DEFAULT_LOCAL_HOST.to_string()
      } else {
        bind_addr.ip().to_string()
      }
    });
  let port = env_u16("PLAYER_SERVER_PUBLIC_PORT")
    .or_else(|| env_u16("GAME_SERVER_PUBLIC_PORT"))
    .or(agones_info.port)
    .unwrap_or_else(|| bind_addr.port());

  ServerEndpoint {
    gs_name,
    namespace,
    ip,
    port,
  }
}

fn player_server_key(prefix: &str, player_id: &str) -> String {
  format!("{prefix}_{player_id}_server")
}

fn first_env(names: &[&str]) -> Option<String> {
  names.iter().find_map(|name| {
    let value = env::var(name).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
  })
}

fn env_u64(name: &str) -> Option<u64> {
  env::var(name).ok()?.trim().parse::<u64>().ok()
}

fn env_u16(name: &str) -> Option<u16> {
  env::var(name).ok()?.trim().parse::<u16>().ok()
}

fn env_bool(name: &str) -> Option<bool> {
  let value = env::var(name).ok()?;
  match value.trim().to_ascii_lowercase().as_str() {
    "1" | "true" | "yes" | "on" => Some(true),
    "0" | "false" | "no" | "off" => Some(false),
    _ => None,
  }
}

fn unix_time_ms() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis() as u64
}

#[cfg(test)]
mod tests {
  use super::player_server_key;

  #[test]
  fn player_server_key_matches_documented_shape() {
    assert_eq!(player_server_key("player", "12345"), "player_12345_server");
  }
}
