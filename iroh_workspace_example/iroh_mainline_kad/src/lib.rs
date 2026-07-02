use std::{
  collections::BTreeMap,
  net::{Ipv4Addr, SocketAddr},
  str::FromStr,
  time::{Duration, SystemTime, UNIX_EPOCH},
};

use iroh::{
  Endpoint, EndpointAddr, EndpointId, RelayMode, RelayUrl, SecretKey, TransportAddr,
  endpoint::{ConnectionError, presets},
};
use mainline::{Dht, Id, MutableItem, SigningKey, Testnet};
use n0_error::{Result, StdResultExt};
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, sleep, timeout};
use tracing::{debug, info, warn};

pub const CLUSTER_ALPN: &[u8] = b"iroh-mainline-kad/cluster/0";
const DEFAULT_CLUSTER_KEY_BYTES: [u8; 32] = [42; 32];
const DEFAULT_CLUSTER_SALT: &[u8] = b"iroh-mainline-kad/v0";
const DHT_VALUE_LIMIT: usize = 1000;

#[derive(Debug, Clone)]
pub struct DhtOptions {
  pub server_mode: bool,
  pub bind: Ipv4Addr,
  pub port: Option<u16>,
  pub bootstrap: Vec<String>,
  pub request_timeout: Duration,
}

impl Default for DhtOptions {
  fn default() -> Self {
    Self {
      server_mode: false,
      bind: Ipv4Addr::UNSPECIFIED,
      port: None,
      bootstrap: Vec::new(),
      request_timeout: Duration::from_secs(4),
    }
  }
}

#[derive(Debug, Clone)]
pub struct IrohOptions {
  pub bind: SocketAddr,
  pub relay: bool,
  pub wait_online: Duration,
}

impl Default for IrohOptions {
  fn default() -> Self {
    Self {
      bind: SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
      relay: true,
      wait_online: Duration::from_secs(15),
    }
  }
}

#[derive(Debug, Clone)]
pub struct ServerOptions {
  pub cluster: ClusterIdentity,
  pub dht: DhtOptions,
  pub iroh: IrohOptions,
  pub name: String,
  pub republish_every: Duration,
}

#[derive(Debug, Clone)]
pub struct ClientOptions {
  pub cluster: ClusterIdentity,
  pub dht: DhtOptions,
  pub iroh: IrohOptions,
  pub message: String,
  pub discover_timeout: Duration,
  pub connect_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct LocalDemoOptions {
  pub dht_nodes: usize,
  pub servers: usize,
  pub message: String,
  pub discover_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct KadServerOptions {
  pub nodes: usize,
  pub bind: Ipv4Addr,
}

#[derive(Debug, Clone)]
pub struct ClusterIdentity {
  signer: SigningKey,
  salt: Vec<u8>,
}

impl ClusterIdentity {
  pub fn from_secret_hex(secret_hex: Option<&str>, salt: impl Into<Vec<u8>>) -> Result<Self> {
    let bytes = match secret_hex {
      Some(value) => parse_secret_key(value)?,
      None => DEFAULT_CLUSTER_KEY_BYTES,
    };

    Ok(Self {
      signer: SigningKey::from_bytes(&bytes),
      salt: salt.into(),
    })
  }

  pub fn public_key(&self) -> [u8; 32] {
    self.signer.verifying_key().to_bytes()
  }

  pub fn salt(&self) -> &[u8] {
    &self.salt
  }

  pub fn target(&self) -> Id {
    MutableItem::target_from_key(&self.public_key(), Some(&self.salt))
  }

  fn signer(&self) -> SigningKey {
    self.signer.clone()
  }
}

impl Default for ClusterIdentity {
  fn default() -> Self {
    Self {
      signer: SigningKey::from_bytes(&DEFAULT_CLUSTER_KEY_BYTES),
      salt: DEFAULT_CLUSTER_SALT.to_vec(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRecord {
  pub version: u8,
  pub updated_at: u64,
  pub members: Vec<MemberRecord>,
}

impl ClusterRecord {
  fn new() -> Self {
    Self {
      version: 1,
      updated_at: now_unix_secs(),
      members: Vec::new(),
    }
  }

  fn insert_member(&mut self, member: MemberRecord, max_members: usize) {
    let mut members = BTreeMap::<String, MemberRecord>::new();

    for existing in self.members.drain(..) {
      members.insert(existing.endpoint_id.clone(), existing);
    }

    members.insert(member.endpoint_id.clone(), member);
    self.members = members.into_values().collect();
    self
      .members
      .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    self.members.truncate(max_members);
    self.updated_at = now_unix_secs();
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRecord {
  pub endpoint_id: String,
  pub name: String,
  pub addrs: Vec<String>,
  pub relay_urls: Vec<String>,
  pub updated_at: u64,
}

impl MemberRecord {
  pub fn endpoint_addr(&self) -> Result<EndpointAddr> {
    let id = EndpointId::from_str(&self.endpoint_id).anyerr()?;
    let mut addrs = Vec::new();

    for addr in &self.addrs {
      addrs.push(TransportAddr::Ip(SocketAddr::from_str(addr).anyerr()?));
    }

    for relay_url in &self.relay_urls {
      addrs.push(TransportAddr::Relay(
        RelayUrl::from_str(relay_url).anyerr()?,
      ));
    }

    Ok(EndpointAddr::from_parts(id, addrs))
  }
}

pub async fn run_server(options: ServerOptions) -> Result<()> {
  let dht = build_dht(&options.dht)?.as_async();
  let endpoint = build_endpoint(&options.iroh, true).await?;
  endpoint_ready(&endpoint, options.iroh.relay, options.iroh.wait_online).await?;

  let member = member_from_endpoint(&endpoint, &options.name);
  println!("iroh endpoint id: {}", member.endpoint_id);
  println!("iroh direct addrs: {}", member.addrs.join(" "));
  if !member.relay_urls.is_empty() {
    println!("iroh relay urls: {}", member.relay_urls.join(" "));
  }

  if let Err(err) = publish_member(&dht, &options.cluster, member, 16).await {
    endpoint.close().await;
    return Err(err);
  }

  let endpoint_for_accept = endpoint.clone();
  let accept_task = tokio::spawn(async move {
    if let Err(err) = accept_loop(endpoint_for_accept).await {
      eprintln!("accept loop stopped: {err:#}");
    }
  });

  let dht_for_publish = dht.clone();
  let cluster_for_publish = options.cluster.clone();
  let endpoint_for_publish = endpoint.clone();
  let name = options.name.clone();
  let republish_every = options.republish_every;
  let republish_task = tokio::spawn(async move {
    loop {
      sleep(republish_every).await;
      let member = member_from_endpoint(&endpoint_for_publish, &name);
      if let Err(err) = publish_member(&dht_for_publish, &cluster_for_publish, member, 16).await {
        warn!("failed to republish cluster member: {err:#}");
      }
    }
  });

  println!(
    "mainline target: {} (salt: {})",
    options.cluster.target(),
    String::from_utf8_lossy(options.cluster.salt())
  );
  println!("server is running. press ctrl-c to stop.");
  tokio::signal::ctrl_c().await.anyerr()?;

  accept_task.abort();
  republish_task.abort();
  endpoint.close().await;
  Ok(())
}

pub async fn run_client(options: ClientOptions) -> Result<()> {
  let dht = build_dht(&options.dht)?.as_async();
  let endpoint = build_endpoint(&options.iroh, false).await?;
  endpoint_ready(&endpoint, options.iroh.relay, options.iroh.wait_online).await?;

  let members = match discover_members(&dht, &options.cluster, options.discover_timeout).await {
    Ok(members) => members,
    Err(err) => {
      endpoint.close().await;
      return Err(err);
    }
  };
  println!("discovered {} member(s)", members.len());

  let mut last_error = None;
  for member in members {
    println!("dialing {} ({})", member.name, member.endpoint_id);
    match timeout(
      options.connect_timeout,
      request(&endpoint, &member, &options.message),
    )
    .await
    {
      Ok(Ok(response)) => {
        println!("response from {}: {response}", member.name);
        endpoint.close().await;
        return Ok(());
      }
      Ok(Err(err)) => {
        warn!("request to {} failed: {err:#}", member.name);
        last_error = Some(err.to_string());
      }
      Err(_) => {
        let msg = format!("request to {} timed out", member.name);
        warn!("{msg}");
        last_error = Some(msg);
      }
    }
  }

  endpoint.close().await;
  Err(n0_error::anyerr!(
    "no discovered member responded: {}",
    last_error.unwrap_or_else(|| "no members".to_string())
  ))
}

pub async fn run_local_demo(options: LocalDemoOptions) -> Result<()> {
  let testnet = Testnet::builder(options.dht_nodes)
    .bind_address(Ipv4Addr::LOCALHOST)
    .build()
    .anyerr()?;

  println!("local mainline bootstrap: {}", testnet.bootstrap.join(" "));

  let cluster = ClusterIdentity::default();
  let mut server_tasks = Vec::with_capacity(options.servers);
  let mut server_endpoints = Vec::with_capacity(options.servers);

  for index in 0..options.servers {
    let dht = build_dht(&DhtOptions {
      server_mode: false,
      bind: Ipv4Addr::LOCALHOST,
      port: None,
      bootstrap: testnet.bootstrap.clone(),
      request_timeout: Duration::from_secs(2),
    })?
    .as_async();

    let endpoint = build_endpoint(
      &IrohOptions {
        bind: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        relay: false,
        wait_online: Duration::from_secs(0),
      },
      true,
    )
    .await?;

    let name = format!("demo-server-{}", index + 1);
    publish_member(&dht, &cluster, member_from_endpoint(&endpoint, &name), 16).await?;

    let endpoint_for_task = endpoint.clone();
    server_tasks.push(tokio::spawn(async move {
      if let Err(err) = accept_loop(endpoint_for_task).await {
        eprintln!("demo accept loop stopped: {err:#}");
      }
    }));
    server_endpoints.push(endpoint);
  }

  let client = ClientOptions {
    cluster,
    dht: DhtOptions {
      server_mode: false,
      bind: Ipv4Addr::LOCALHOST,
      port: None,
      bootstrap: testnet.bootstrap.clone(),
      request_timeout: Duration::from_secs(2),
    },
    iroh: IrohOptions {
      bind: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
      relay: false,
      wait_online: Duration::from_secs(0),
    },
    message: options.message,
    discover_timeout: options.discover_timeout,
    connect_timeout: Duration::from_secs(8),
  };

  let result = run_client(client).await;

  for endpoint in server_endpoints {
    endpoint.close().await;
  }
  for task in server_tasks {
    task.abort();
  }

  result
}

pub async fn run_kad_server(options: KadServerOptions) -> Result<()> {
  let testnet = Testnet::builder(options.nodes)
    .bind_address(options.bind)
    .build()
    .anyerr()?;

  let bootstrap = testnet.bootstrap.join(",");
  println!("mainline kad testnet is running");
  println!("export KAD_BOOTSTRAP={bootstrap}");
  println!("nodes:");
  for address in &testnet.bootstrap {
    println!("  {address}");
  }
  println!("keep this process running while servers and clients use the bootstrap list");

  tokio::signal::ctrl_c().await.anyerr()?;
  drop(testnet);
  Ok(())
}

pub fn default_cluster_salt() -> Vec<u8> {
  DEFAULT_CLUSTER_SALT.to_vec()
}

fn build_dht(options: &DhtOptions) -> Result<Dht> {
  let mut builder = Dht::builder();

  builder
    .bind_address(options.bind)
    .request_timeout(options.request_timeout);

  if options.server_mode {
    builder.server_mode();
  }

  if let Some(port) = options.port {
    builder.port(port);
  }

  if !options.bootstrap.is_empty() {
    builder.bootstrap(&options.bootstrap);
  }

  builder.build().anyerr()
}

async fn build_endpoint(options: &IrohOptions, accept: bool) -> Result<Endpoint> {
  let mut builder = Endpoint::builder(presets::N0)
    .secret_key(SecretKey::generate())
    .relay_mode(if options.relay {
      RelayMode::Default
    } else {
      RelayMode::Disabled
    });

  if accept {
    builder = builder.alpns(vec![CLUSTER_ALPN.to_vec()]);
  }

  builder = builder
    .clear_ip_transports()
    .bind_addr(options.bind)
    .anyerr()?;
  builder.bind().await.anyerr()
}

async fn endpoint_ready(endpoint: &Endpoint, relay: bool, wait_online: Duration) -> Result<()> {
  if relay && !wait_online.is_zero() {
    timeout(wait_online, endpoint.online()).await.anyerr()?;
  }
  Ok(())
}

async fn publish_member(
  dht: &mainline::async_dht::AsyncDht,
  cluster: &ClusterIdentity,
  member: MemberRecord,
  max_members: usize,
) -> Result<()> {
  let mut last_error = None;

  for attempt in 1..=3 {
    match publish_member_once(dht, cluster, member.clone(), max_members).await {
      Ok(()) => return Ok(()),
      Err(err) => {
        last_error = Some(err.to_string());
        if attempt < 3 {
          sleep(Duration::from_millis(250 * attempt)).await;
        }
      }
    }
  }

  Err(n0_error::anyerr!(
    "failed to publish cluster member after retries: {}",
    last_error.unwrap_or_else(|| "unknown error".to_string())
  ))
}

async fn publish_member_once(
  dht: &mainline::async_dht::AsyncDht,
  cluster: &ClusterIdentity,
  member: MemberRecord,
  max_members: usize,
) -> Result<()> {
  let public_key = cluster.public_key();
  let salt = Some(cluster.salt());
  let current = dht.get_mutable_most_recent(&public_key, salt).await;

  let mut record = match current.as_ref() {
    Some(item) => serde_json::from_slice::<ClusterRecord>(item.value()).anyerr()?,
    None => ClusterRecord::new(),
  };
  let cas = current.as_ref().map(MutableItem::seq);
  let seq = current.as_ref().map_or(1, |item| item.seq() + 1);

  record.insert_member(member, max_members);

  let mut value = serde_json::to_vec(&record).anyerr()?;
  while value.len() > DHT_VALUE_LIMIT && record.members.len() > 1 {
    record.members.pop();
    value = serde_json::to_vec(&record).anyerr()?;
  }

  if value.len() > DHT_VALUE_LIMIT {
    return Err(n0_error::anyerr!(
      "cluster record is {} bytes, exceeding mainline BEP44 limit of {} bytes",
      value.len(),
      DHT_VALUE_LIMIT
    ));
  }

  let item = MutableItem::new(cluster.signer(), &value, seq, salt);
  let outcome = dht.put_mutable(item, cas).await.anyerr()?;
  println!(
    "published cluster record seq={seq} target={} stored_at={}",
    outcome.target, outcome.stored_at
  );

  Ok(())
}

async fn discover_members(
  dht: &mainline::async_dht::AsyncDht,
  cluster: &ClusterIdentity,
  discover_timeout: Duration,
) -> Result<Vec<MemberRecord>> {
  let public_key = cluster.public_key();
  let salt = Some(cluster.salt());

  let deadline = Instant::now() + discover_timeout;
  loop {
    if let Some(item) = dht.get_mutable_most_recent(&public_key, salt).await {
      let mut record = serde_json::from_slice::<ClusterRecord>(item.value()).anyerr()?;
      record
        .members
        .sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

      if !record.members.is_empty() {
        return Ok(record.members);
      }
    }

    if Instant::now() >= deadline {
      return Err(n0_error::anyerr!(
        "no cluster members found at target {}",
        cluster.target()
      ));
    }

    sleep(Duration::from_millis(500)).await;
  }
}

async fn request(endpoint: &Endpoint, member: &MemberRecord, message: &str) -> Result<String> {
  let addr = member.endpoint_addr()?;
  let conn = endpoint.connect(addr, CLUSTER_ALPN).await?;
  let (mut send, mut recv) = conn.open_bi().await.anyerr()?;

  send.write_all(message.as_bytes()).await.anyerr()?;
  send.finish().anyerr()?;

  let response = recv.read_to_end(1024).await.anyerr()?;
  let response = String::from_utf8(response).anyerr()?;
  conn.close(0u32.into(), b"done");
  Ok(response)
}

async fn accept_loop(endpoint: Endpoint) -> Result<()> {
  while let Some(incoming) = endpoint.accept().await {
    let accepting = match incoming.accept() {
      Ok(accepting) => accepting,
      Err(err) => {
        warn!("incoming connection failed: {err:#}");
        continue;
      }
    };

    let me = endpoint.id();
    tokio::spawn(async move {
      if let Err(err) = handle_connection(accepting, me).await {
        warn!("connection handler failed: {err:#}");
      }
    });
  }

  Ok(())
}

async fn handle_connection(mut accepting: iroh::endpoint::Accepting, me: EndpointId) -> Result<()> {
  let alpn = accepting.alpn().await?;
  let conn = accepting.await?;
  let remote = conn.remote_id();
  info!(
    "accepted {} from {}",
    String::from_utf8_lossy(&alpn),
    remote
  );

  let (mut send, mut recv) = conn.accept_bi().await.anyerr()?;
  let message = recv.read_to_end(1024).await.anyerr()?;
  let message = String::from_utf8(message).anyerr()?;
  println!("received from {remote}: {message}");

  let response = format!("hi from {me}; received: {message}");
  send.write_all(response.as_bytes()).await.anyerr()?;
  send.finish().anyerr()?;

  let closed = timeout(Duration::from_secs(3), conn.closed()).await;
  if let Ok(closed) = closed {
    if !matches!(closed, ConnectionError::ApplicationClosed(_)) {
      debug!("remote {remote} closed with: {closed:#}");
    }
  }

  Ok(())
}

fn member_from_endpoint(endpoint: &Endpoint, name: &str) -> MemberRecord {
  let addr = endpoint.addr();
  let addrs = addr.ip_addrs().map(ToString::to_string).collect::<Vec<_>>();
  let relay_urls = addr
    .relay_urls()
    .map(ToString::to_string)
    .collect::<Vec<_>>();

  MemberRecord {
    endpoint_id: endpoint.id().to_string(),
    name: name.to_string(),
    addrs,
    relay_urls,
    updated_at: now_unix_secs(),
  }
}

fn parse_secret_key(input: &str) -> Result<[u8; 32]> {
  let input = input.trim();
  let mut bytes = [0u8; 32];

  if input.len() != 64 {
    return Err(n0_error::anyerr!(
      "cluster secret must be 64 hex characters"
    ));
  }

  for (index, chunk) in input.as_bytes().chunks_exact(2).enumerate() {
    let part = std::str::from_utf8(chunk).anyerr()?;
    bytes[index] = u8::from_str_radix(part, 16).anyerr()?;
  }

  Ok(bytes)
}

fn now_unix_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
}

pub fn parse_bootstrap(values: &[String]) -> Vec<String> {
  values
    .iter()
    .flat_map(|value| value.split(','))
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(ToString::to_string)
    .collect()
}

pub fn parse_duration_secs(seconds: u64) -> Duration {
  Duration::from_secs(seconds)
}

pub fn parse_socket_addr(value: &str) -> Result<SocketAddr> {
  SocketAddr::from_str(value).anyerr()
}

pub fn parse_ipv4(value: &str) -> Result<Ipv4Addr> {
  Ipv4Addr::from_str(value).anyerr()
}

pub fn parse_dht_port(port: u16) -> Option<u16> {
  (port != 0).then_some(port)
}
