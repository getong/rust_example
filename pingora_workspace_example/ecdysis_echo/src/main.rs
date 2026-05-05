use std::{
  env, fs, io,
  net::{IpAddr, SocketAddr},
  os::unix::{fs::FileTypeExt, net::UnixStream as StdUnixStream},
  path::Path,
  str::FromStr,
  time::{Duration, Instant},
};

use ecdysis::tokio_ecdysis::{SignalKind, StopOnShutdown, TokioEcdysisBuilder};
use futures::{Stream, StreamExt};
use nix::unistd::getpid;
use tokio::{
  io::{AsyncRead, AsyncWrite, AsyncWriteExt, copy, split},
  net::{TcpStream, UnixStream},
};

// Exit the process after this Duration, even if tasks remain.
const EXIT_AFTER_UPGRADE_DURATION: Duration = std::time::Duration::from_secs(60);

pub trait AsyncReadWrite: AsyncRead + AsyncWrite {}
impl AsyncReadWrite for TcpStream {}
impl AsyncReadWrite for UnixStream {}

async fn echo_server<S, C>(mut sock_stream: S)
where
  S: Stream<Item = io::Result<C>> + Unpin + Sized,
  C: AsyncReadWrite + Unpin + Send + 'static,
{
  // Whenever a new connection is established, the first message sent by this echo server
  // corresponds to a single uint32 indicating the number of reloads so far.

  let reload_count = env::var("ECDYSIS_RELOADS").unwrap().parse::<u32>().unwrap();

  // TODO: Consider using "Structured Concurrency" when that lands:
  // https://github.com/tokio-rs/tokio/issues/1879
  let wg = waitgroup::WaitGroup::new();

  while let Some(Ok(mut client)) = sock_stream.next().await {
    let w = wg.worker();
    let client_fut = async move {
      client.write_all(&reload_count.to_be_bytes()).await.unwrap();
      let (mut client_r, mut client_w) = split(client);
      let _n_bytes = copy(&mut client_r, &mut client_w).await;

      drop(w); // Task finished
    };
    tokio::spawn(client_fut);
  }

  wg.wait().await;
}

fn set_process_env_var(key: &str, value: impl AsRef<std::ffi::OsStr>) {
  // SAFETY: This binary mutates process environment only during startup on the
  // current-thread runtime, before it spawns any tasks or worker threads.
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
      format!(
        "control socket {} is already in use by a running process",
        path.display()
      ),
    )),
    Err(err)
      if matches!(
        err.kind(),
        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
      ) =>
    {
      fs::remove_file(path)?;
      log::info!("Removed stale control socket {}", path.display());
      Ok(())
    }
    Err(err) => Err(io::Error::new(
      err.kind(),
      format!("failed to probe control socket {}: {err}", path.display()),
    )),
  }
}

#[cfg(feature = "systemd_sockets")]
fn should_try_systemd_sockets(reload_count: u32) -> bool {
  reload_count > 0
    || env::var_os("LISTEN_PID").is_some()
    || env::var_os("LISTEN_FDNAMES").is_some()
    || env::var_os("LISTEN_FDS").is_some()
}

// single thread needed for set_listen_pid
#[tokio::main(flavor = "current_thread")]
async fn main() {
  env_logger::init_from_env(
    env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
  );
  set_listen_pid();

  let reload_count = match env::var("ECDYSIS_RELOADS") {
    Err(_) => {
      set_process_env_var("ECDYSIS_RELOADS", "0");
      0
    }
    Ok(s) => {
      let reload_count = s.parse::<u32>().unwrap() + 1;
      set_process_env_var("ECDYSIS_RELOADS", reload_count.to_string());
      reload_count
    }
  };

  let pid = getpid();
  if reload_count == 0 {
    log::info!("echo server parent started (PID: {pid})");
  } else {
    log::info!("echo server child started (PID: {pid}; reload count: {reload_count})");
  }

  let mut ecdysis_builder = TokioEcdysisBuilder::new(SignalKind::hangup()).unwrap();
  prepare_control_socket("/tmp/ecdysis_upgrade.sock").unwrap();
  prepare_control_socket("/tmp/ecdysis_exit.sock").unwrap();
  prepare_control_socket("/tmp/ecdysis_partial_exit.sock").unwrap();
  ecdysis_builder
    .stop_on_signal(SignalKind::user_defined1())
    .unwrap();
  ecdysis_builder
    .partial_stop_on_signal(SignalKind::user_defined2())
    .unwrap();
  ecdysis_builder
    .upgrade_on_socket("/tmp/ecdysis_upgrade.sock")
    .unwrap();
  ecdysis_builder
    .stop_on_socket("/tmp/ecdysis_exit.sock")
    .unwrap();
  ecdysis_builder
    .partial_stop_on_socket("/tmp/ecdysis_partial_exit.sock")
    .unwrap();
  ecdysis_builder.set_pid_file("./pidfile");
  #[cfg(feature = "systemd_notify")]
  if let Err(err) = ecdysis_builder.enable_systemd_notifications() {
    log::info!("Failed to enable systemd notifications: {err}");
  }

  #[cfg(feature = "systemd_sockets")]
  let should_try_systemd = should_try_systemd_sockets(reload_count);

  #[cfg(feature = "systemd_sockets")]
  if should_try_systemd {
    if let Err(err) = ecdysis_builder.read_systemd_sockets() {
      log::info!("Systemd sockets unavailable, skipping systemd listeners: {err:?}");
    }
  }

  let ip_addr = match IpAddr::from_str("[::1]") {
    Ok(ip_addr) => ip_addr,
    Err(_) => IpAddr::from_str("0.0.0.0").unwrap(),
  };
  let addr = SocketAddr::new(ip_addr, 22222);

  log::info!("Address is: {:?}", addr);

  let stream = ecdysis_builder
    .build_listen_tcp(StopOnShutdown::Yes, addr, |b, addr| {
      if ip_addr.is_ipv6() {
        b.set_only_v6(true).expect("cannot set v6 here");
      }
      b.set_reuse_address(true).expect("Cannot set REUSEADDR");
      b.bind(&addr.into()).expect("Cannot bind to provided IP");
      b.listen(128)?;
      Ok(b.into())
    })
    .unwrap();
  let server_handle = tokio::spawn(echo_server(stream));

  #[cfg(feature = "systemd_sockets")]
  let systemd_server_handle = if should_try_systemd {
    match ecdysis_builder
      .systemd_listen_unix(
        StopOnShutdown::Yes,
        "ecdysis_test_unix".to_string(),
        "/tmp/ecdysis_int_test.sock".to_string(),
      )
      .await
    {
      Ok(sd_unix_stream) => Some(tokio::spawn(echo_server(sd_unix_stream))),
      Err(err) => {
        log::info!("Systemd unix socket unavailable, skipping listener: {err:?}");
        None
      }
    }
  } else {
    None
  };

  let (_tokio_ecdysis, ecdysis_fut) = ecdysis_builder.ready().unwrap();

  let exit = ecdysis_fut.await;
  log::info!("Shutdown because: {:?}", exit);
  let exit_start_time = Instant::now();

  // Service existing conections for up to 1 minute
  tokio::spawn(async {
    tokio::time::sleep(EXIT_AFTER_UPGRADE_DURATION).await;
    log::info!(
      "Force-exiting {:?} after upgrade",
      EXIT_AFTER_UPGRADE_DURATION
    );
    std::process::exit(1)
  });

  // add a forceful 1 second sleep here to be able to properly test synchronous shutdowns
  tokio::time::sleep(Duration::from_secs(1)).await;

  server_handle.await.unwrap();
  #[cfg(feature = "systemd_sockets")]
  if let Some(systemd_server_handle) = systemd_server_handle {
    systemd_server_handle.await.unwrap();
  }
  log::info!(
    "Graceful exit {:?} after ecdysis stop (reason: {exit:?})",
    exit_start_time.elapsed()
  );
}

fn set_listen_pid() {
  // ideally systemd should set LISTEN_PID and LISTEN_FDNAMES to read systemd activated sockets.
  // but for our tests, LISTEN_PID cant be set because there is not easy way to set the pid
  // before the test process is exec-ed.
  // So this is a hack, where LISTEN_PID will be set if LISTEN_FDNAMES is set.
  //
  // set_var is undefined in multithreaded environment. So echo_server uses single thread
  // environment.

  match env::var("LISTEN_FDNAMES") {
    Ok(v) => {
      if v.is_empty() {
        return;
      }
    }
    Err(_) => return,
  }

  // a non empty LISTEN_FDNAMES value was set, so write LISTEN_PID too
  let pid = format!("{}", std::process::id());
  set_process_env_var("LISTEN_PID", pid);
  log::info!("LISTEN_PID updated");
}
