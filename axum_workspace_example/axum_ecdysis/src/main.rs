use std::{
  convert::Infallible,
  env, fs, io,
  net::SocketAddr,
  os::unix::{fs::FileTypeExt, net::UnixStream as StdUnixStream},
  path::Path,
  time::{Duration, Instant},
};

use axum::{
  Json, Router,
  extract::{ConnectInfo, Request, State},
  response::IntoResponse,
  routing::get,
};
use ecdysis::tokio_ecdysis::{SignalKind, StopOnShutdown, TokioEcdysisBuilder};
use futures::{Stream, StreamExt};
use hyper::body::Incoming;
use hyper_util::{
  rt::{TokioExecutor, TokioIo},
  server,
};
use nix::unistd::getpid;
use serde::Serialize;
use tokio::net::TcpStream;
use tower::{Service, ServiceExt};

const EXIT_AFTER_UPGRADE_DURATION: Duration = Duration::from_secs(60);
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:3000";
const PID_FILE: &str = "./pidfile";
const UPGRADE_SOCKET_PATH: &str = "/tmp/axum_ecdysis_upgrade.sock";
const STOP_SOCKET_PATH: &str = "/tmp/axum_ecdysis_exit.sock";
const PARTIAL_STOP_SOCKET_PATH: &str = "/tmp/axum_ecdysis_partial_exit.sock";

#[derive(Clone)]
struct AppState {
  pid: u32,
  reload_count: u32,
  started_at: Instant,
}

#[derive(Serialize)]
struct StatusResponse {
  message: &'static str,
  pid: u32,
  reload_count: u32,
  uptime_secs: u64,
  remote_addr: String,
}

#[derive(Serialize)]
struct SlowResponse {
  message: &'static str,
  pid: u32,
  reload_count: u32,
  slept_secs: u64,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
  env_logger::init_from_env(
    env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
  );
  set_listen_pid();

  let reload_count = update_reload_count()?;
  let pid = std::process::id();

  if reload_count == 0 {
    log::info!("axum_ecdysis parent started (PID: {pid})");
  } else {
    log::info!("axum_ecdysis child started (PID: {pid}; reload count: {reload_count})");
  }

  let bind_addr = parse_bind_addr()?;
  let app_state = AppState {
    pid,
    reload_count,
    started_at: Instant::now(),
  };
  let app = build_app(app_state);

  let mut ecdysis_builder = TokioEcdysisBuilder::new(SignalKind::hangup())?;
  if reload_count == 0 {
    prepare_control_socket(UPGRADE_SOCKET_PATH)?;
    prepare_control_socket(STOP_SOCKET_PATH)?;
    prepare_control_socket(PARTIAL_STOP_SOCKET_PATH)?;
  }

  ecdysis_builder.stop_on_signal(SignalKind::user_defined1())?;
  ecdysis_builder.partial_stop_on_signal(SignalKind::user_defined2())?;
  ecdysis_builder.upgrade_on_socket(UPGRADE_SOCKET_PATH)?;
  ecdysis_builder.stop_on_socket(STOP_SOCKET_PATH)?;
  ecdysis_builder.partial_stop_on_socket(PARTIAL_STOP_SOCKET_PATH)?;
  ecdysis_builder.set_pid_file(PID_FILE);

  #[cfg(feature = "systemd_notify")]
  if let Err(err) = ecdysis_builder.enable_systemd_notifications() {
    log::info!("failed to enable systemd notifications: {err}");
  }

  #[cfg(feature = "systemd_sockets")]
  let should_try_systemd = should_try_systemd_sockets(reload_count);

  #[cfg(feature = "systemd_sockets")]
  if should_try_systemd {
    if let Err(err) = ecdysis_builder.read_systemd_sockets() {
      log::info!("systemd sockets unavailable, skipping: {err:?}");
    }
  }

  let listener_stream =
    ecdysis_builder.build_listen_tcp(StopOnShutdown::Yes, bind_addr, |builder, addr| {
      if addr.is_ipv6() {
        builder.set_only_v6(true).expect("cannot set IPV6_V6ONLY");
      }
      builder
        .set_reuse_address(true)
        .expect("cannot set SO_REUSEADDR");
      builder
        .bind(&addr.into())
        .expect("cannot bind listen address");
      builder.listen(1024)?;
      Ok(builder.into())
    })?;

  log::info!("HTTP server listening on {bind_addr}");
  log::info!("upgrade socket: {UPGRADE_SOCKET_PATH}");
  log::info!("stop socket: {STOP_SOCKET_PATH}");
  log::info!("partial stop socket: {PARTIAL_STOP_SOCKET_PATH}");

  let server_handle = tokio::spawn(run_axum(listener_stream, app));

  let (_tokio_ecdysis, ecdysis_fut) = ecdysis_builder.ready()?;
  let exit = ecdysis_fut.await;
  let exit_start_time = Instant::now();
  log::info!("shutdown triggered: {exit:?}");

  tokio::spawn(async {
    tokio::time::sleep(EXIT_AFTER_UPGRADE_DURATION).await;
    log::error!(
      "force-exiting after {:?} waiting for draining connections",
      EXIT_AFTER_UPGRADE_DURATION
    );
    std::process::exit(1);
  });

  server_handle.await??;

  log::info!(
    "graceful exit after {:?} (reason: {exit:?})",
    exit_start_time.elapsed()
  );
  Ok(())
}

fn build_app(state: AppState) -> Router {
  Router::new()
    .route("/", get(root))
    .route("/healthz", get(healthz))
    .route("/slow/{secs}", get(slow))
    .with_state(state)
}

async fn root(
  State(state): State<AppState>,
  ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
  Json(StatusResponse {
    message: "axum + ecdysis is running",
    pid: state.pid,
    reload_count: state.reload_count,
    uptime_secs: state.started_at.elapsed().as_secs(),
    remote_addr: remote_addr.to_string(),
  })
}

async fn healthz() -> &'static str {
  "ok"
}

async fn slow(
  State(state): State<AppState>,
  axum::extract::Path(secs): axum::extract::Path<u64>,
) -> impl IntoResponse {
  let slept_secs = secs.min(30);
  tokio::time::sleep(Duration::from_secs(slept_secs)).await;

  Json(SlowResponse {
    message: "completed after delay",
    pid: state.pid,
    reload_count: state.reload_count,
    slept_secs,
  })
}

async fn run_axum<S>(mut listener_stream: S, app: Router) -> io::Result<()>
where
  S: Stream<Item = io::Result<TcpStream>> + Unpin,
{
  let mut make_service = app.into_make_service_with_connect_info::<SocketAddr>();
  let connection_wait_group = waitgroup::WaitGroup::new();

  while let Some(stream_result) = listener_stream.next().await {
    let stream = match stream_result {
      Ok(stream) => stream,
      Err(err) => {
        log::error!("accept failed: {err}");
        continue;
      }
    };

    let remote_addr = match stream.peer_addr() {
      Ok(addr) => addr,
      Err(err) => {
        log::error!("failed to read peer address: {err}");
        continue;
      }
    };

    let tower_service = unwrap_infallible(make_service.call(remote_addr).await);
    let wait_worker = connection_wait_group.worker();

    tokio::spawn(async move {
      let socket = TokioIo::new(stream);
      let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
        tower_service.clone().oneshot(request)
      });

      if let Err(err) = server::conn::auto::Builder::new(TokioExecutor::new())
        .serve_connection_with_upgrades(socket, hyper_service)
        .await
      {
        log::error!("connection error for {remote_addr}: {err:#}");
      }

      drop(wait_worker);
    });
  }

  connection_wait_group.wait().await;
  Ok(())
}

fn parse_bind_addr() -> io::Result<SocketAddr> {
  let bind_addr = env::var("APP_ADDR").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned());
  bind_addr.parse::<SocketAddr>().map_err(|err| {
    io::Error::new(
      io::ErrorKind::InvalidInput,
      format!("invalid APP_ADDR `{bind_addr}`: {err}"),
    )
  })
}

fn update_reload_count() -> io::Result<u32> {
  let reload_count = match env::var("ECDYSIS_RELOADS") {
    Err(_) => 0,
    Ok(value) => {
      value.parse::<u32>().map_err(|err| {
        io::Error::new(
          io::ErrorKind::InvalidData,
          format!("invalid ECDYSIS_RELOADS `{value}`: {err}"),
        )
      })?
        + 1
    }
  };

  set_process_env_var("ECDYSIS_RELOADS", reload_count.to_string());
  Ok(reload_count)
}

fn set_process_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
  // SAFETY: This process only mutates environment variables during startup on the
  // current-thread runtime, before any request tasks are spawned.
  unsafe { env::set_var(key, value) };
}

fn prepare_control_socket(path: impl AsRef<Path>) -> io::Result<()> {
  let path = path.as_ref();

  let metadata = match fs::symlink_metadata(path) {
    Ok(metadata) => metadata,
    Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
    Err(err) => return Err(err),
  };

  if !metadata.file_type().is_socket() {
    return Err(io::Error::new(
      io::ErrorKind::AlreadyExists,
      format!(
        "control socket path {} exists and is not a socket",
        path.display()
      ),
    ));
  }

  match StdUnixStream::connect(path) {
    Ok(_) => Err(io::Error::new(
      io::ErrorKind::AddrInUse,
      format!("control socket {} is already in use", path.display()),
    )),
    Err(err)
      if matches!(
        err.kind(),
        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
      ) =>
    {
      fs::remove_file(path)?;
      log::info!("removed stale control socket {}", path.display());
      Ok(())
    }
    Err(err) => Err(io::Error::new(
      err.kind(),
      format!("failed to probe control socket {}: {err}", path.display()),
    )),
  }
}

fn unwrap_infallible<T>(result: Result<T, Infallible>) -> T {
  match result {
    Ok(value) => value,
    Err(err) => match err {},
  }
}

fn set_listen_pid() {
  match env::var("LISTEN_FDNAMES") {
    Ok(value) if !value.is_empty() => {}
    _ => return,
  }

  set_process_env_var("LISTEN_PID", std::process::id().to_string());
  log::info!("LISTEN_PID updated");
}

#[cfg(feature = "systemd_sockets")]
fn should_try_systemd_sockets(reload_count: u32) -> bool {
  reload_count > 0
    || env::var_os("LISTEN_PID").is_some()
    || env::var_os("LISTEN_FDNAMES").is_some()
    || env::var_os("LISTEN_FDS").is_some()
}

#[allow(dead_code)]
fn _log_pid_for_debugging() {
  let pid = getpid();
  log::debug!("current pid: {pid}");
}
