use std::{
  path::Path,
  sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
  },
  time::Duration,
};

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use clickhouse::{Client, Compression, Row, sql};
use dotenvy::from_filename;
use hyper_rustls::HttpsConnectorBuilder;
use hyper_util::{
  client::legacy::{Client as HyperClient, connect::HttpConnector},
  rt::TokioExecutor,
};
use rustls::{ClientConfig, RootCertStore, crypto::aws_lc_rs};
use rustls_native_certs::load_native_certs;
use rustls_pemfile::certs;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use spider::{Client as SpiderClient, page::Page};

#[derive(Debug, Row, Serialize, Deserialize)]
struct Product {
  url: String,
  image: String,
  name: String,
  price: String,
}

#[derive(Clone)]
struct AppState {
  pool: Arc<ClientPool>,
  primary_idx: usize,
}

struct ClientPool {
  clients: Vec<Client>,
  cursor: AtomicUsize,
}

impl ClientPool {
  fn new(clients: Vec<Client>) -> Self {
    assert!(
      !clients.is_empty(),
      "at least one ClickHouse node URL is required"
    );
    Self {
      clients,
      cursor: AtomicUsize::new(0),
    }
  }

  fn all(&self) -> &[Client] {
    &self.clients
  }

  /// Round-robin pick of the next client; returns (node_idx, Client).
  fn next(&self) -> (usize, Client) {
    let current = self.cursor.fetch_add(1, Ordering::Relaxed);
    let idx = current % self.clients.len();
    (idx, self.clients[idx].clone())
  }

  /// Get a specific client by index; clamps to the first client if out of bounds.
  fn get(&self, idx: usize) -> Client {
    let safe_idx = if idx < self.clients.len() { idx } else { 0 };
    self.clients[safe_idx].clone()
  }
}

fn env_first<'a>(keys: &[&'a str], default: &str) -> String {
  keys
    .iter()
    .find_map(|k| std::env::var(k).ok())
    .unwrap_or_else(|| default.to_string())
}

fn read_ca_path() -> Option<String> {
  let path = env_first(&["CLICKHOUSE_CA_CERT", "CH_CA_CERT"], "tls/ca.crt");
  if Path::new(&path).exists() {
    Some(path)
  } else {
    eprintln!("Warning: CA certificate not found at {path}; falling back to native roots only");
    None
  }
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
  let native = load_native_certs();
  if !native.errors.is_empty() {
    eprintln!(
      "Warning: failed to load some native certs: {:?}",
      native.errors
    );
  }
  for cert in native.certs {
    roots.add(cert)?;
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
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Load .env if present for local runs (silently ignores missing file).
  let _ = from_filename(".env");
  let _ = aws_lc_rs::default_provider().install_default();

  let target_url = "https://www.scrapingcourse.com/ecommerce/";

  // Fetch the page HTML with spider's HTTP client.
  let spider_client = SpiderClient::builder()
    .user_agent("spider-clickhouse-example/0.1")
    .build()?;
  let page = Page::new_page(target_url, &spider_client).await;
  let html = page.get_html();

  // Parse products from the HTML.
  let document = Html::parse_document(&html);
  let product_selector = Selector::parse("li.product").expect("valid product selector");
  let link_selector = Selector::parse("a").expect("valid link selector");
  let image_selector = Selector::parse("img").expect("valid img selector");
  let name_selector = Selector::parse("h2").expect("valid name selector");
  let price_selector = Selector::parse(".price").expect("valid price selector");

  let mut products = Vec::new();

  for product_el in document.select(&product_selector) {
    let url = product_el
      .select(&link_selector)
      .next()
      .and_then(|link| link.value().attr("href"))
      .unwrap_or("")
      .to_string();

    let image = product_el
      .select(&image_selector)
      .next()
      .and_then(|img| {
        img
          .value()
          .attr("src")
          .or_else(|| img.value().attr("data-src"))
          .or_else(|| img.value().attr("data-lazy-src"))
      })
      .unwrap_or("")
      .to_string();

    let name = product_el
      .select(&name_selector)
      .next()
      .map(flatten_text)
      .unwrap_or_default();

    let price = product_el
      .select(&price_selector)
      .next()
      .map(flatten_text)
      .unwrap_or_default();

    if url.is_empty() && image.is_empty() && name.is_empty() && price.is_empty() {
      continue;
    }

    products.push(Product {
      url,
      image,
      name,
      price,
    });
  }

  // Set up ClickHouse connection and write scraped data to a table using the cluster-style
  // connection pattern (CLICKHOUSE_* envs first, CH_* fallbacks, optional node list).
  let url = env_first(&["CLICKHOUSE_URL", "CH_URL"], "https://localhost:8443");
  let user = env_first(&["CLICKHOUSE_USER", "CH_USER"], "default");
  let password = env_first(&["CLICKHOUSE_PASSWORD", "CH_PASSWORD"], "changeme");
  let database = env_first(&["CLICKHOUSE_DATABASE", "CH_DB"], "spider");
  let node_urls = env_first(
    &["CLICKHOUSE_NODES", "CH_NODES"],
    &format!("{url},https://localhost:8444,https://localhost:8445,https://localhost:8446"),
  );
  let ca_cert = read_ca_path();

  // First, create the database using clients without a default database set to avoid
  // UNKNOWN_DATABASE errors.
  let base_pool = ClientPool::new(build_clients(
    &node_urls,
    &user,
    &password,
    None,
    ca_cert.as_deref(),
  )?);
  for client in base_pool.all() {
    client
      .query("CREATE DATABASE IF NOT EXISTS ?")
      .bind(sql::Identifier(&database))
      .execute()
      .await?;
  }

  // Rebuild pool with the target database attached for table creation and DML.
  let pool = Arc::new(ClientPool::new(build_clients(
    &node_urls,
    &user,
    &password,
    Some(&database),
    ca_cert.as_deref(),
  )?));

  // Ensure database and table exist on every configured node to avoid missing-table errors
  // when a different node is selected via round-robin.
  for client in pool.all() {
    // Database now exists but keep the creation idempotent.
    client
      .query("CREATE DATABASE IF NOT EXISTS ?")
      .bind(sql::Identifier(&database))
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

  // Use round-robin pick for writing/reading; defaults to the first node when only one is set.
  let (writer_idx, ch_client) = pool.next();

  let mut insert = ch_client.insert::<Product>("products").await?;
  for product in products {
    insert.write(&product).await?;
  }
  insert.end().await?;

  // Read back inserted rows so runs show scraped data directly.
  let stored = ch_client
    .query("SELECT ?fields FROM products ORDER BY name")
    .fetch_all::<Product>()
    .await?;

  println!("Read {} products from ClickHouse:", stored.len());
  for product in stored {
    println!("{:?}", product);
  }

  // Serve data via Axum so it can be consumed externally.
  let app_state = AppState {
    pool: pool.clone(),
    primary_idx: writer_idx,
  };
  let router = Router::new()
    .route("/products", get(list_products))
    .with_state(app_state);

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
  println!("Axum server running at http://0.0.0.0:3000/products");

  axum::serve(listener, router).await?;

  Ok(())
}

fn flatten_text(el: ElementRef<'_>) -> String {
  el.text().collect::<Vec<_>>().join("").trim().to_string()
}

async fn list_products(
  State(state): State<AppState>,
) -> Result<Json<Vec<Product>>, (StatusCode, String)> {
  // Always read from the node that received the writes to avoid empty results when the pool
  // round-robins across non-replicated nodes.
  let client = state.pool.get(state.primary_idx);
  client
    .query("SELECT ?fields FROM products ORDER BY name")
    .fetch_all::<Product>()
    .await
    .map(Json)
    .map_err(internal_error)
}

fn internal_error<E: std::fmt::Display>(err: E) -> (StatusCode, String) {
  (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}
