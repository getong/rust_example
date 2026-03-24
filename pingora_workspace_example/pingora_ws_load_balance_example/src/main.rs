use std::{env, sync::Arc, time::Duration};

use async_trait::async_trait;
use clap::Parser;
use log::info;
use pingora_core::{
  Result,
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

fn main() {
  env_logger::init();

  let opt = Opt::parse();
  let mut server = Server::new(Some(opt)).expect("failed to create Pingora server");
  server.bootstrap();

  let listen_addr = env::var("WS_LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:6188".into());
  let upstreams = parse_upstreams_from_env();
  let upstream_tls = parse_bool_env("WS_UPSTREAM_TLS", false);
  let upstream_sni = env::var("WS_UPSTREAM_SNI").unwrap_or_default();
  assert!(
    !upstreams.is_empty(),
    "WS_UPSTREAMS cannot be empty. Example: WS_UPSTREAMS=127.0.0.1:9001,127.0.0.1:9002"
  );

  info!("listen_addr={listen_addr}");
  info!("upstreams={upstreams:?}");
  info!("upstream_tls={upstream_tls}, upstream_sni={upstream_sni}");

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
  proxy.add_tcp(&listen_addr);

  server.add_service(proxy);
  server.add_service(background);
  server.run_forever();
}
