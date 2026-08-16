use std::{
  net::{IpAddr, Ipv4Addr, SocketAddr},
  process,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use bevy::{
  app::{AppExit, ScheduleRunnerPlugin},
  ecs::message::MessageWriter,
  prelude::*,
  state::app::StatesPlugin,
};
use game_client::protocol::{
  DEFAULT_SERVER_ADDR, GameProtocolPlugin, NETCODE_PRIVATE_KEY, NETCODE_PROTOCOL_ID,
};
use game_shared::replication::GameReplicationPlugin;
use lightyear::prelude::{
  Authentication, Client, Connected, Disconnected, LocalAddr, UdpIo,
  client::{ClientPlugins, Connect, NetcodeClient, NetcodeConfig},
};

#[derive(Resource)]
struct ProbeState {
  server_addr: SocketAddr,
  timeout: Timer,
  complete: bool,
}

fn main() {
  let server_addr = match configured_server_addr() {
    Ok(server_addr) => server_addr,
    Err(err) => {
      eprintln!("invalid server address: {err}");
      process::exit(2);
    }
  };
  let timeout_seconds = std::env::var("GAME_CLIENT_PROBE_TIMEOUT_SECONDS")
    .ok()
    .and_then(|value| value.parse::<f32>().ok())
    .filter(|seconds| *seconds > 0.0)
    .unwrap_or(5.0);

  App::new()
    .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(16))))
    .add_plugins(StatesPlugin)
    .add_plugins(ClientPlugins {
      tick_duration: Duration::from_secs_f64(1.0 / 30.0),
    })
    .add_plugins(GameProtocolPlugin)
    .add_plugins(GameReplicationPlugin)
    .insert_resource(ProbeState {
      server_addr,
      timeout: Timer::from_seconds(timeout_seconds, TimerMode::Once),
      complete: false,
    })
    .add_systems(Startup, start_probe)
    .add_systems(Update, check_probe)
    .run();
}

fn start_probe(
  mut commands: Commands,
  state: Res<ProbeState>,
  mut app_exit: MessageWriter<AppExit>,
) {
  let auth = Authentication::Manual {
    server_addr: state.server_addr,
    client_id: configured_client_id(),
    private_key: NETCODE_PRIVATE_KEY,
    protocol_id: NETCODE_PROTOCOL_ID,
  };
  let netcode_client = match NetcodeClient::new(auth, NetcodeConfig::default()) {
    Ok(client) => client,
    Err(err) => {
      eprintln!("network init failed: {err}");
      app_exit.write(AppExit::from_code(2));
      return;
    }
  };

  let local_addr = local_client_addr(state.server_addr);
  println!(
    "probing Lightyear server {}, local bind {}",
    state.server_addr, local_addr
  );
  let entity = commands
    .spawn((UdpIo::default(), LocalAddr(local_addr), netcode_client))
    .id();
  commands.trigger(Connect { entity });
}

fn check_probe(
  time: Res<Time>,
  mut state: ResMut<ProbeState>,
  mut app_exit: MessageWriter<AppExit>,
  client_query: Query<(Option<&Connected>, Option<&Disconnected>), With<Client>>,
) {
  if state.complete {
    return;
  }

  if let Ok((connected, disconnected)) = client_query.single() {
    if connected.is_some() {
      println!("connected to {}", state.server_addr);
      state.complete = true;
      app_exit.write(AppExit::Success);
      return;
    }

    if let Some(disconnected) = disconnected {
      eprintln!(
        "disconnected from {}: {}",
        state.server_addr,
        disconnected.reason
      );
      state.complete = true;
      app_exit.write(AppExit::from_code(1));
      return;
    }
  }

  state.timeout.tick(time.delta());
  if state.timeout.is_finished() {
    eprintln!("connection probe timed out for {}", state.server_addr);
    state.complete = true;
    app_exit.write(AppExit::from_code(1));
  }
}

fn configured_server_addr() -> Result<SocketAddr, std::net::AddrParseError> {
  let server_addr = std::env::var("GAME_SERVER_ADDR")
    .unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string())
    .parse()?;

  rewrite_local_agones_addr(server_addr)
}

fn local_client_addr(server_addr: SocketAddr) -> SocketAddr {
  if server_addr.is_ipv4() {
    SocketAddr::from(([0, 0, 0, 0], 0))
  } else {
    SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 0], 0))
  }
}

fn rewrite_local_agones_addr(
  server_addr: SocketAddr,
) -> Result<SocketAddr, std::net::AddrParseError> {
  if let Ok(host) = std::env::var("GAME_SERVER_PUBLIC_HOST") {
    let host = host.trim();
    if !host.is_empty() {
      return Ok(SocketAddr::new(host.parse::<IpAddr>()?, server_addr.port()));
    }
  }

  if should_use_local_agones_forward(server_addr) {
    return Ok(SocketAddr::new(
      IpAddr::V4(Ipv4Addr::LOCALHOST),
      server_addr.port(),
    ));
  }

  Ok(server_addr)
}

fn should_use_local_agones_forward(server_addr: SocketAddr) -> bool {
  env_bool("GAME_SERVER_AGONES_LOCALHOST").unwrap_or(false)
    && is_docker_desktop_node_ip(server_addr.ip())
}

fn is_docker_desktop_node_ip(ip: IpAddr) -> bool {
  let IpAddr::V4(ip) = ip else {
    return false;
  };
  let [first, second, ..] = ip.octets();
  first == 172 && (16 ..= 31).contains(&second)
}

fn env_bool(name: &str) -> Option<bool> {
  let value = std::env::var(name).ok()?;
  match value.trim().to_ascii_lowercase().as_str() {
    "1" | "true" | "yes" | "on" => Some(true),
    "0" | "false" | "no" | "off" => Some(false),
    _ => None,
  }
}

fn configured_client_id() -> u64 {
  if let Ok(client_id) = std::env::var("GAME_CLIENT_ID") {
    if let Ok(client_id) = client_id.parse::<u64>() {
      return client_id;
    }
  }

  let micros = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |duration| duration.as_micros() as u64);
  micros ^ u64::from(process::id())
}
