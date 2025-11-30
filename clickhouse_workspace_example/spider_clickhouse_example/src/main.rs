use clickhouse::{Client, Compression, Row, sql};
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

fn env_first<'a>(keys: &[&'a str], default: &str) -> String {
  keys
    .iter()
    .find_map(|k| std::env::var(k).ok())
    .unwrap_or_else(|| default.to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

  // Set up ClickHouse connection and write scraped data to a table. Prefer official env
  // names (CLICKHOUSE_*) with CH_* fallbacks to mirror cluster example behavior.
  let url = env_first(&["CLICKHOUSE_URL", "CH_URL"], "http://localhost:8123");
  let user = env_first(&["CLICKHOUSE_USER", "CH_USER"], "default");
  let password = env_first(&["CLICKHOUSE_PASSWORD", "CH_PASSWORD"], "changeme");
  let database = env_first(&["CLICKHOUSE_DATABASE", "CH_DB"], "spider");

  let base_client = Client::default()
    .with_url(url)
    .with_user(user)
    .with_password(password)
    .with_compression(Compression::Lz4);

  base_client
    .query("CREATE DATABASE IF NOT EXISTS ?")
    .bind(sql::Identifier(&database))
    .execute()
    .await?;

  let ch_client = base_client.with_database(database.clone());

  ch_client
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

  Ok(())
}

fn flatten_text(el: ElementRef<'_>) -> String {
  el.text().collect::<Vec<_>>().join("").trim().to_string()
}
