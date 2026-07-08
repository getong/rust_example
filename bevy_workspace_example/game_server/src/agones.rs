use std::{env, thread, time::Duration};

use bevy::prelude::*;
use lightyear::prelude::server::Started;
use tokio::sync::mpsc;

const DEFAULT_HEALTH_INTERVAL: Duration = Duration::from_secs(2);
const DEFAULT_KEEP_ALIVE: Duration = Duration::from_secs(30);

pub(crate) struct AgonesPlugin;

impl Plugin for AgonesPlugin {
  fn build(&self, app: &mut App) {
    app
      .init_resource::<AgonesBridge>()
      .init_resource::<AgonesGameServerInfo>()
      .add_systems(Startup, start_agones_bridge)
      .add_systems(Update, (drain_agones_updates, notify_agones_ready).chain());
  }
}

#[derive(Resource, Clone, Debug, Default)]
pub(crate) struct AgonesGameServerInfo {
  pub(crate) name: Option<String>,
  pub(crate) namespace: Option<String>,
  pub(crate) address: Option<String>,
  pub(crate) port: Option<u16>,
}

#[derive(Resource)]
struct AgonesBridge {
  sender: Option<mpsc::UnboundedSender<AgonesCommand>>,
  updates: Option<mpsc::UnboundedReceiver<AgonesGameServerInfo>>,
  ready_on_start: bool,
  ready_sent: bool,
}

impl Default for AgonesBridge {
  fn default() -> Self {
    Self {
      sender: None,
      updates: None,
      ready_on_start: true,
      ready_sent: false,
    }
  }
}

#[derive(Debug)]
enum AgonesCommand {
  Ready,
}

#[derive(Debug)]
struct AgonesSettings {
  enabled: bool,
  ready_on_start: bool,
  watch_gameserver: bool,
  health_interval: Duration,
  ready_delay: Duration,
  keep_alive: Duration,
  labels: Vec<(String, String)>,
  annotations: Vec<(String, String)>,
}

impl AgonesSettings {
  fn from_env() -> Self {
    let enabled = env_bool("AGONES_ENABLED").unwrap_or_else(has_agones_sidecar_env);
    let health_interval = duration_from_env("AGONES_HEALTH_INTERVAL_SECONDS")
      .filter(|duration| !duration.is_zero())
      .unwrap_or(DEFAULT_HEALTH_INTERVAL);

    Self {
      enabled,
      ready_on_start: env_bool("AGONES_READY_ON_START").unwrap_or(true),
      watch_gameserver: env_bool("AGONES_WATCH_GAMESERVER").unwrap_or(false),
      health_interval,
      ready_delay: duration_from_env("AGONES_READY_DELAY_SECONDS").unwrap_or_default(),
      keep_alive: duration_from_env("AGONES_KEEP_ALIVE_SECONDS").unwrap_or(DEFAULT_KEEP_ALIVE),
      labels: pairs_from_env("AGONES_LABELS"),
      annotations: pairs_from_env("AGONES_ANNOTATIONS"),
    }
  }
}

fn start_agones_bridge(mut bridge: ResMut<AgonesBridge>) {
  let settings = AgonesSettings::from_env();
  bridge.ready_on_start = settings.ready_on_start;
  let (sender, updates) = spawn_agones_manager(settings);
  bridge.sender = sender;
  bridge.updates = updates;
}

fn drain_agones_updates(
  mut bridge: ResMut<AgonesBridge>,
  mut game_server_info: ResMut<AgonesGameServerInfo>,
) {
  let Some(updates) = bridge.updates.as_mut() else {
    return;
  };

  while let Ok(update) = updates.try_recv() {
    *game_server_info = update;
  }
}

fn notify_agones_ready(
  mut bridge: ResMut<AgonesBridge>,
  started_servers: Query<(), Added<Started>>,
) {
  if !bridge.ready_on_start || bridge.ready_sent || started_servers.is_empty() {
    return;
  }

  let Some(sender) = bridge.sender.as_ref() else {
    return;
  };

  match sender.send(AgonesCommand::Ready) {
    Ok(()) => {
      bridge.ready_sent = true;
    }
    Err(err) => {
      eprintln!("game_server agones ready notification failed: {err}");
    }
  }
}

fn spawn_agones_manager(
  settings: AgonesSettings,
) -> (
  Option<mpsc::UnboundedSender<AgonesCommand>>,
  Option<mpsc::UnboundedReceiver<AgonesGameServerInfo>>,
) {
  if !settings.enabled {
    println!("game_server agones integration disabled");
    return (None, None);
  }

  println!(
    "game_server agones integration enabled, health_interval={:?}, ready_on_start={}",
    settings.health_interval, settings.ready_on_start
  );

  let (sender, receiver) = mpsc::unbounded_channel();
  let (updates_sender, updates_receiver) = mpsc::unbounded_channel();

  match thread::Builder::new()
    .name("agones-sdk".to_string())
    .spawn(move || {
      let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
      {
        Ok(runtime) => runtime,
        Err(err) => {
          eprintln!("game_server agones runtime error: {err}");
          return;
        }
      };

      runtime.block_on(run_agones_manager(settings, receiver, updates_sender));
    }) {
    Ok(_thread) => (Some(sender), Some(updates_receiver)),
    Err(err) => {
      eprintln!("game_server agones thread start error: {err}");
      (None, None)
    }
  }
}

async fn run_agones_manager(
  settings: AgonesSettings,
  mut receiver: mpsc::UnboundedReceiver<AgonesCommand>,
  updates: mpsc::UnboundedSender<AgonesGameServerInfo>,
) {
  println!("game_server agones sdk connecting");
  let mut sdk = match ::agones::Sdk::new_with_host(None, None, Some(settings.keep_alive)).await {
    Ok(sdk) => sdk,
    Err(err) => {
      eprintln!("game_server agones sdk connect error: {err}");
      return;
    }
  };
  println!("game_server agones sdk connected");

  spawn_health_task(&sdk, settings.health_interval);
  match sdk.get_gameserver().await {
    Ok(game_server) => publish_gameserver_update(&updates, &game_server),
    Err(err) => eprintln!("game_server agones get gameserver error: {err}"),
  }
  if settings.watch_gameserver {
    spawn_gameserver_watch(sdk.clone(), updates.clone());
  }
  apply_metadata(&mut sdk, &settings).await;

  while let Some(command) = receiver.recv().await {
    match command {
      AgonesCommand::Ready => mark_ready(&mut sdk, &settings).await,
    }
  }
}

fn spawn_health_task(sdk: &::agones::Sdk, health_interval: Duration) {
  let health_sender = sdk.health_check();
  tokio::task::spawn(async move {
    let mut interval = tokio::time::interval(health_interval);

    loop {
      interval.tick().await;
      if health_sender.send(()).await.is_err() {
        eprintln!("game_server agones health stream closed");
        break;
      }
    }
  });
}

fn spawn_gameserver_watch(
  mut sdk: ::agones::Sdk,
  updates: mpsc::UnboundedSender<AgonesGameServerInfo>,
) {
  tokio::task::spawn(async move {
    println!("game_server agones gameserver watch starting");
    let mut stream = match sdk.watch_gameserver().await {
      Ok(stream) => stream,
      Err(err) => {
        eprintln!("game_server agones gameserver watch error: {err}");
        return;
      }
    };

    loop {
      match stream.message().await {
        Ok(Some(game_server)) => {
          publish_gameserver_update(&updates, &game_server);
          let name = game_server
            .object_meta
            .as_ref()
            .map(|metadata| metadata.name.as_str())
            .unwrap_or("<unknown>");
          let state = game_server
            .status
            .as_ref()
            .map(|status| status.state.as_str())
            .unwrap_or("<unknown>");
          println!("game_server agones gameserver update: name={name}, state={state}");
        }
        Ok(None) => {
          println!("game_server agones gameserver watch closed");
          break;
        }
        Err(err) => {
          eprintln!("game_server agones gameserver watch stream error: {err}");
          break;
        }
      }
    }
  });
}

fn publish_gameserver_update(
  updates: &mpsc::UnboundedSender<AgonesGameServerInfo>,
  game_server: &::agones::GameServer,
) {
  let _ = updates.send(gameserver_info(game_server));
}

fn gameserver_info(game_server: &::agones::GameServer) -> AgonesGameServerInfo {
  let (name, namespace) = game_server
    .object_meta
    .as_ref()
    .map(|metadata| {
      (
        Some(metadata.name.clone()),
        Some(metadata.namespace.clone()),
      )
    })
    .unwrap_or_default();
  let (address, port) = game_server
    .status
    .as_ref()
    .map(|status| {
      (
        non_empty(status.address.as_str()).or_else(|| {
          status
            .addresses
            .iter()
            .find_map(|address| non_empty(address.address.as_str()))
        }),
        status
          .ports
          .iter()
          .find(|port| port.name == "game")
          .or_else(|| status.ports.first())
          .and_then(|port| u16::try_from(port.port).ok()),
      )
    })
    .unwrap_or_default();

  AgonesGameServerInfo {
    name,
    namespace,
    address,
    port,
  }
}

fn non_empty(value: &str) -> Option<String> {
  let value = value.trim();
  (!value.is_empty()).then(|| value.to_string())
}

async fn apply_metadata(sdk: &mut ::agones::Sdk, settings: &AgonesSettings) {
  for (key, value) in &settings.labels {
    if let Err(err) = sdk.set_label(key, value).await {
      eprintln!("game_server agones set label {key} error: {err}");
    }
  }

  for (key, value) in &settings.annotations {
    if let Err(err) = sdk.set_annotation(key, value).await {
      eprintln!("game_server agones set annotation {key} error: {err}");
    }
  }
}

async fn mark_ready(sdk: &mut ::agones::Sdk, settings: &AgonesSettings) {
  if !settings.ready_delay.is_zero() {
    tokio::time::sleep(settings.ready_delay).await;
  }

  match sdk.ready().await {
    Ok(()) => println!("game_server agones marked ready"),
    Err(err) => eprintln!("game_server agones ready error: {err}"),
  }
}

fn has_agones_sidecar_env() -> bool {
  env::var_os("AGONES_SDK_GRPC_HOST").is_some() || env::var_os("AGONES_SDK_GRPC_PORT").is_some()
}

fn env_bool(name: &str) -> Option<bool> {
  let value = env::var(name).ok()?;
  match value.trim().to_ascii_lowercase().as_str() {
    "1" | "true" | "yes" | "on" => Some(true),
    "0" | "false" | "no" | "off" => Some(false),
    "auto" => None,
    _ => None,
  }
}

fn duration_from_env(name: &str) -> Option<Duration> {
  let seconds = env::var(name).ok()?.trim().parse::<u64>().ok()?;
  Some(Duration::from_secs(seconds))
}

fn pairs_from_env(name: &str) -> Vec<(String, String)> {
  let Ok(value) = env::var(name) else {
    return Vec::new();
  };

  value
    .split(',')
    .filter_map(|entry| {
      let (key, value) = entry.split_once('=')?;
      let key = key.trim();
      if key.is_empty() {
        return None;
      }
      Some((key.to_string(), value.trim().to_string()))
    })
    .collect()
}
