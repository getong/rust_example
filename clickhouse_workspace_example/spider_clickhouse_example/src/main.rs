mod clickhouse_layer;
mod redis_cluster;

use std::{
  path::Path,
  sync::Arc,
  time::{SystemTime, UNIX_EPOCH},
};

use ::clickhouse as clickhouse_driver;
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use clickhouse_driver::Row;
use dotenvy::from_filename;
use redis_cluster::{RedisSettings, redis_connection, set_string, REDIS_CONN};
use reqwest::Client as HttpClient;
use rustls::crypto::aws_lc_rs;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use spider::{Client as SpiderClient, page::Page};
use url::Url;

use crate::clickhouse_layer::{ClickhouseSettings, ClientPool, clickhouse_state};

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

fn log_html_diagnostics(
  source: &str,
  base_url: &Url,
  status: Option<reqwest::StatusCode>,
  html: &str,
) {
  let snippet = html
    .chars()
    .take(160)
    .collect::<String>()
    .replace('\n', " ");
  let contains_cf = html.to_ascii_lowercase().contains("just a moment")
    || html.contains("cf-chl-")
    || html.contains("cloudflare");
  let contains_products = html.contains("class=\"product\"");
  println!(
    "[{}] base_url={} status={:?} bytes={} contains_products={} cloudflare_challenge={} \
     snippet=\"{}\"",
    source,
    base_url,
    status,
    html.len(),
    contains_products,
    contains_cf,
    snippet
  );
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

fn ensure_tls_dir() -> Result<(), Box<dyn std::error::Error>> {
  let tls_dir = Path::new("tls");
  if tls_dir.exists() {
    return Ok(());
  }

  eprintln!("TLS directory missing, running ./generate_tls.sh ...");
  // let status = Command::new("./generate_tls.sh").status()?;
  // if !status.success() {
  //   return Err("generate_tls.sh failed".into());
  // }

  // if !tls_dir.exists() {
  //   return Err("TLS directory still missing after generate_tls.sh".into());
  // }
  Err("please run generate_tls.sh".into())
}

fn absolute_url(base: &Url, value: &str) -> String {
  if value.starts_with("http://") || value.starts_with("https://") {
    return value.to_string();
  }

  base
    .join(value.trim_start_matches('/'))
    .map(|u| u.to_string())
    .unwrap_or_else(|_| value.to_string())
}

fn parse_products(html: &str, base_url: &Url) -> Vec<Product> {
  // BooksToScrape layout:
  // <article class="product_pod">
  //   <div class="image_container"><a><img src="..."/></a></div>
  //   <h3><a title="NAME" href="catalogue/..."></a></h3>
  //   <p class="price_color">£xx.xx</p>
  // </article>
  let document = Html::parse_document(html);
  let product_selector = Selector::parse("article.product_pod").expect("valid product selector");
  let link_selector = Selector::parse("h3 a").expect("valid link selector");
  let image_selector = Selector::parse("div.image_container img").expect("valid image selector");
  let price_selector = Selector::parse("p.price_color").expect("valid price selector");

  let mut products = Vec::new();

  for product_el in document.select(&product_selector) {
    let url = product_el
      .select(&link_selector)
      .next()
      .and_then(|link| link.value().attr("href"))
      .map(|href| absolute_url(base_url, href))
      .unwrap_or_else(String::new);

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
      .map(|src| absolute_url(base_url, src))
      .unwrap_or_else(String::new);

    let name = product_el
      .select(&link_selector)
      .next()
      .and_then(|link| link.value().attr("title"))
      .map(|t| t.to_string())
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

  products
}

async fn fetch_html_with_spider(target_url: &str) -> Result<String, Box<dyn std::error::Error>> {
  let spider_client = SpiderClient::builder()
    .user_agent("spider-clickhouse-example/0.1")
    .build()?;
  let page = Page::new_page(target_url, &spider_client).await;
  Ok(page.get_html())
}

async fn fetch_html_with_reqwest(
  target_url: &str,
) -> Result<(String, Url, reqwest::StatusCode), Box<dyn std::error::Error>> {
  let client = HttpClient::builder()
    .user_agent("spider-clickhouse-example/0.1")
    .build()?;
  let resp = client.get(target_url).send().await?;
  let status = resp.status();
  let final_url = resp.url().clone();
  let html = resp.error_for_status()?.text().await?;
  Ok((html, final_url, status))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Load .env if present for local runs (silently ignores missing file).
  let _ = from_filename(".env");
  let _ = aws_lc_rs::default_provider().install_default();
  ensure_tls_dir()?;

  // let target_url = "https://www.scrapingcourse.com/ecommerce/";
  let target_url = "https://books.toscrape.com/";
  let base_url = Url::parse(target_url)?;

  // Fetch the page HTML with spider's HTTP client; if it returns zero products (e.g., blocked),
  // retry with a plain reqwest client.
  let html = fetch_html_with_spider(target_url).await?;
  log_html_diagnostics("spider", &base_url, None, &html);
  let mut products = parse_products(&html, &base_url);

  if products.is_empty() {
    eprintln!("Spider fetch returned 0 products; retrying with reqwest...");
    if let Ok((fallback_html, fallback_base_url, status)) =
      fetch_html_with_reqwest(target_url).await
    {
      log_html_diagnostics("reqwest", &fallback_base_url, Some(status), &fallback_html);
      let fallback_products = parse_products(&fallback_html, &fallback_base_url);
      if !fallback_products.is_empty() {
        products = fallback_products;
      } else {
        eprintln!("Fallback fetch also returned 0 products; check target page markup.");
      }
    }
  }

  // Configuration for downstream storage/cache layers.
  let url = env_first(&["CLICKHOUSE_URL", "CH_URL"], "https://localhost:8443");
  let user = env_first(&["CLICKHOUSE_USER", "CH_USER"], "default");
  let password = env_first(&["CLICKHOUSE_PASSWORD", "CH_PASSWORD"], "changeme");
  let database = env_first(&["CLICKHOUSE_DATABASE", "CH_DB"], "spider");
  let node_urls = env_first(
    &["CLICKHOUSE_NODES", "CH_NODES"],
    &format!("{url},https://localhost:8444,https://localhost:8445,https://localhost:8446"),
  );
  let ca_cert = read_ca_path();
  let redis_nodes = env_first(
    &["REDIS_NODES", "REDIS_CLUSTER_NODES"],
    "redis://localhost:7000,redis://localhost:7001,redis://localhost:7002,redis://localhost:7003,\
     redis://localhost:7004,redis://localhost:7005",
  );
  let redis_timeout_ms: u64 = env_first(&["REDIS_CONNECT_TIMEOUT_MS"], "5000")
    .parse()
    .unwrap_or(5000);
  let redis_optional = env_first(&["REDIS_OPTIONAL"], "false")
    .parse::<bool>()
    .unwrap_or(false);

  // Initialize ClickHouse once for the process (global state via once_cell).
  let ch_state = clickhouse_state(ClickhouseSettings {
    node_urls,
    user,
    password,
    database,
    ca_cert,
  })
  .await?;

  // Initialize Redis once for the process (global state via once_cell).
  let redis_conn_opt = match redis_connection(&RedisSettings {
    nodes: redis_nodes,
    timeout_ms: redis_timeout_ms,
  })
  .await
  {
    Ok(_) => REDIS_CONN.get().cloned(),
    Err(err) if redis_optional => {
      eprintln!("⚠ Redis unavailable ({}); continuing without Redis", err);
      None
    }
    Err(err) => return Err(err),
  };

  // Record the scrape run metadata in Redis for quick inspection (if connected).
  if let Some(redis_conn) = redis_conn_opt.as_ref() {
    let last_run_ts = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_secs()
      .to_string();
    set_string(redis_conn, "spider:last_run", last_run_ts).await?;
    set_string(redis_conn, "spider:last_target", target_url).await?;
  }

  // Use round-robin pick for writing/reading; defaults to the first node when only one is set.
  let writer_idx = ch_state.primary_idx;
  let ch_client = ch_state.primary_client();

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
  for product in &stored {
    println!("{:?}", product);
  }

  // Cache the number of scraped products so external tools can read a cheap heartbeat (if
  // connected).
  if let Some(redis_conn) = redis_conn_opt.as_ref() {
    set_string(
      redis_conn,
      "spider:last_product_count",
      stored.len().to_string(),
    )
    .await?;
  }

  // Serve data via Axum so it can be consumed externally.
  let app_state = AppState {
    pool: ch_state.pool.clone(),
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
