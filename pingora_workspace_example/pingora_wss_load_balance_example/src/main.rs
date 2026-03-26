use std::{env, path::Path, sync::Arc, time::Duration};

use async_trait::async_trait;
use clap::Parser;
use log::info;
use pingora_core::{
  Result,
  listeners::{ALPN, tls::TlsSettings},
  server::{Server, configuration::Opt},
  services::background::background_service,
  upstreams::peer::HttpPeer,
};
use pingora_load_balancing::{LoadBalancer, health_check, selection::RoundRobin};
use pingora_proxy::{ProxyHttp, Session};

struct WsLb {
  upstreams: Arc<LoadBalancer<RoundRobin>>,
  upstream_tls: bool,
  upstream_sni: String,
}

const DEFAULT_TLS_CERT_PATH: &str = "certs/localhost-cert.pem";
const DEFAULT_TLS_KEY_PATH: &str = "certs/localhost-key.pem";

#[async_trait]
impl ProxyHttp for WsLb {
  type CTX = ();

  fn new_ctx(&self) -> Self::CTX {}

  async fn upstream_peer(
    &self,
    session: &mut Session,
    _ctx: &mut Self::CTX,
  ) -> Result<Box<HttpPeer>> {
    let upstream = self
      .upstreams
      .select(b"", 256)
      .expect("no healthy upstream available");

    let is_ws_upgrade = session
      .req_header()
      .headers
      .get("upgrade")
      .is_some_and(|value| value.as_bytes().eq_ignore_ascii_case(b"websocket"));

    info!(
      "selected upstream: {:?}, websocket_upgrade={is_ws_upgrade}",
      upstream
    );

    let mut peer = Box::new(HttpPeer::new(
      upstream,
      self.upstream_tls,
      self.upstream_sni.clone(),
    ));
    // WebSocket handshake (Upgrade) relies on HTTP/1.1.
    peer.options.set_http_version(1, 1);
    Ok(peer)
  }
}

fn parse_upstreams_from_env() -> Vec<String> {
  let raw = env::var("WS_UPSTREAMS").unwrap_or_else(|_| "127.0.0.1:9001,127.0.0.1:9002".into());
  raw
    .split(',')
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(ToOwned::to_owned)
    .collect()
}

fn parse_healthcheck_secs_from_env() -> u64 {
  env::var("WS_HEALTHCHECK_INTERVAL_SECS")
    .ok()
    .and_then(|value| value.parse::<u64>().ok())
    .filter(|value| *value > 0)
    .unwrap_or(1)
}

fn parse_bool_env(name: &str, default: bool) -> bool {
  env::var(name)
    .ok()
    .map(|value| {
      matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
      )
    })
    .unwrap_or(default)
}

fn parse_optional_env(name: &str) -> Option<String> {
  env::var(name)
    .ok()
    .map(|value| value.trim().to_owned())
    .filter(|value| !value.is_empty())
}

fn resolve_tls_file_path(env_name: &str, default: &str) -> String {
  parse_optional_env(env_name).unwrap_or_else(|| default.to_owned())
}

fn main() {
  env_logger::init();

  let opt = Opt::parse();
  let mut server = Server::new(Some(opt)).expect("failed to create Pingora server");
  server.bootstrap();

  let listen_addr = env::var("WS_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:6188".into());
  let upstreams = parse_upstreams_from_env();
  let upstream_tls = parse_bool_env("WS_UPSTREAM_TLS", false);
  let upstream_sni = env::var("WS_UPSTREAM_SNI").unwrap_or_default();
  let downstream_tls = parse_bool_env("WS_DOWNSTREAM_TLS", false);
  let tls_cert_path = resolve_tls_file_path("WS_TLS_CERT_PATH", DEFAULT_TLS_CERT_PATH);
  let tls_key_path = resolve_tls_file_path("WS_TLS_KEY_PATH", DEFAULT_TLS_KEY_PATH);
  assert!(
    !upstreams.is_empty(),
    "WS_UPSTREAMS cannot be empty. Example: WS_UPSTREAMS=127.0.0.1:9001,127.0.0.1:9002"
  );

  info!("listen_addr={listen_addr}");
  info!("upstreams={upstreams:?}");
  info!("upstream_tls={upstream_tls}, upstream_sni={upstream_sni}");
  info!("downstream_tls={downstream_tls}");
  info!("tls_cert_path={tls_cert_path}, tls_key_path={tls_key_path}");

  let mut lb = LoadBalancer::try_from_iter(upstreams.iter().map(String::as_str))
    .expect("invalid WS_UPSTREAMS, expected comma-separated host:port list");

  let hc = health_check::TcpHealthCheck::new();
  lb.set_health_check(hc);
  lb.health_check_frequency = Some(Duration::from_secs(parse_healthcheck_secs_from_env()));

  let background = background_service("ws upstream health check", lb);
  let upstreams = background.task();

  let mut proxy = pingora_proxy::http_proxy_service(
    &server.configuration,
    WsLb {
      upstreams,
      upstream_tls,
      upstream_sni,
    },
  );
  if downstream_tls {
    assert!(
      Path::new(&tls_cert_path).is_file(),
      "TLS cert file not found: {}. Run: ./scripts/generate-dev-cert.sh",
      tls_cert_path
    );
    assert!(
      Path::new(&tls_key_path).is_file(),
      "TLS key file not found: {}. Run: ./scripts/generate-dev-cert.sh",
      tls_key_path
    );

    let mut tls_settings = TlsSettings::intermediate(&tls_cert_path, &tls_key_path)
      .expect("failed to build TLS settings from WS_TLS_CERT_PATH/WS_TLS_KEY_PATH");
    // WebSocket handshake still requires HTTP/1.1 over TLS (wss).
    tls_settings.set_alpn(ALPN::H1);
    proxy.add_tls_with_settings(&listen_addr, None, tls_settings);
    info!("listening mode=wss");
  } else {
    proxy.add_tcp(&listen_addr);
    info!("listening mode=ws");
  }

  server.add_service(proxy);
  server.add_service(background);
  server.run_forever();
}
