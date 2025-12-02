use std::{
  panic::catch_unwind,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use clickhouse::{Client, Compression, sql};
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{
  client::legacy::{Client as HyperClient, connect::HttpConnector},
  rt::TokioExecutor,
};
use once_cell::sync::Lazy;
use rustls::{ClientConfig, RootCertStore};
use rustls_native_certs::load_native_certs;
use rustls_pemfile::certs;
use tokio::sync::OnceCell;

#[derive(Clone)]
pub struct ClickhouseSettings {
  pub node_urls: String,
  pub user: String,
  pub password: String,
  pub database: String,
  pub ca_cert: Option<String>,
}

pub struct ClickhouseState {
  pub pool: Arc<ClientPool>,
  pub primary_idx: usize,
}

pub struct ClientPool {
  clients: Vec<Client>,
  cursor: AtomicUsize,
}

impl ClientPool {
  pub fn new(clients: Vec<Client>) -> Self {
    assert!(
      !clients.is_empty(),
      "at least one ClickHouse node URL is required"
    );
    Self {
      clients,
      cursor: AtomicUsize::new(0),
    }
  }

  pub fn all(&self) -> &[Client] {
    &self.clients
  }

  /// Round-robin pick of the next client; returns (node_idx, Client).
  pub fn next(&self) -> (usize, Client) {
    let current = self.cursor.fetch_add(1, Ordering::Relaxed);
    let idx = current % self.clients.len();
    (idx, self.clients[idx].clone())
  }

  /// Get a specific client by index; clamps to the first client if out of bounds.
  pub fn get(&self, idx: usize) -> Client {
    let safe_idx = if idx < self.clients.len() { idx } else { 0 };
    self.clients[safe_idx].clone()
  }
}

impl ClickhouseState {
  pub fn primary_client(&self) -> Client {
    self.pool.get(self.primary_idx)
  }
}

type SharedState = Arc<ClickhouseState>;

static CLICKHOUSE_STATE: Lazy<OnceCell<SharedState>> = Lazy::new(|| OnceCell::const_new());

pub async fn clickhouse_state(
  settings: ClickhouseSettings,
) -> Result<SharedState, Box<dyn std::error::Error>> {
  CLICKHOUSE_STATE
    .get_or_try_init(|| async move { build_state(settings).await })
    .await
    .map(Arc::clone)
}

async fn build_state(
  settings: ClickhouseSettings,
) -> Result<SharedState, Box<dyn std::error::Error>> {
  let base_pool = ClientPool::new(build_clients(
    &settings.node_urls,
    &settings.user,
    &settings.password,
    None,
    settings.ca_cert.as_deref(),
  )?);
  for client in base_pool.all() {
    client
      .query("CREATE DATABASE IF NOT EXISTS ?")
      .bind(sql::Identifier(&settings.database))
      .execute()
      .await?;
  }

  let pool = Arc::new(ClientPool::new(build_clients(
    &settings.node_urls,
    &settings.user,
    &settings.password,
    Some(&settings.database),
    settings.ca_cert.as_deref(),
  )?));

  for client in pool.all() {
    client
      .query("CREATE DATABASE IF NOT EXISTS ?")
      .bind(sql::Identifier(&settings.database))
      .execute()
      .await?;

    client
      .query(
        "
          CREATE TABLE IF NOT EXISTS products (
            url String,
            image String,
            name String,
            price String
          )
          ENGINE = MergeTree
          ORDER BY (url)
        ",
      )
      .execute()
      .await?;
  }

  let (primary_idx, _) = pool.next();
  Ok(Arc::new(ClickhouseState { pool, primary_idx }))
}

fn build_clients(
  urls: &str,
  user: &str,
  password: &str,
  db: Option<&str>,
  ca_cert: Option<&str>,
) -> Result<Vec<Client>, Box<dyn std::error::Error>> {
  let mut connector = HttpConnector::new();
  connector.set_keepalive(Some(Duration::from_secs(60)));
  connector.enforce_http(false);

  let mut roots = RootCertStore::empty();
  if let Some(native) = load_native_roots_safely() {
    if !native.errors.is_empty() {
      eprintln!(
        "Warning: failed to load some native certs: {:?}",
        native.errors
      );
    }
    for cert in native.certs {
      roots.add(cert)?;
    }
  }

  if let Some(path) = ca_cert {
    let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut found = 0;
    for cert in certs(&mut reader) {
      roots.add(cert?)?;
      found += 1;
    }

    if found == 0 {
      return Err(format!("no certificates found in {path}").into());
    }
  }

  let tls = ClientConfig::builder()
    .with_root_certificates(roots)
    .with_no_client_auth();

  let https = HttpsConnectorBuilder::new()
    .with_tls_config(tls)
    .https_or_http()
    .enable_http1()
    .wrap_connector(connector);

  let transport = HyperClient::builder(TokioExecutor::new())
    .pool_idle_timeout(Duration::from_secs(2))
    .build(https);

  let clients = urls
    .split(',')
    .filter(|s| !s.trim().is_empty())
    .map(|url| {
      let client = Client::with_http_client(transport.clone())
        .with_url(url.trim())
        .with_user(user)
        .with_password(password)
        .with_compression(Compression::Lz4);

      if let Some(db_name) = db {
        client.with_database(db_name.to_string())
      } else {
        client
      }
    })
    .collect::<Vec<_>>();

  Ok(clients)
}

fn load_native_roots_safely() -> Option<rustls_native_certs::CertificateResult> {
  match catch_unwind(|| load_native_certs()) {
    Ok(result) => Some(result),
    Err(_) => {
      eprintln!("Warning: native certificate store is unavailable; proceeding with custom CA only");
      None
    }
  }
}
