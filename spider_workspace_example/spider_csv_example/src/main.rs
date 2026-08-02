use scraper::{ElementRef, Html, Selector};
use spider::{page::Page, tokio, Client};

#[derive(Debug)]
struct Product {
  url: String,
  image: String,
  name: String,
  price: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let target_url = "https://www.scrapingcourse.com/ecommerce/";

  // Fetch the page HTML with spider's HTTP client.
  let client = Client::builder()
    .user_agent("spider-csv-example/0.1")
    .build()?;
  let page = Page::new_page(target_url, &client).await;
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

  // Write to CSV to mirror the headless_chrome example output.
  let mut writer = csv::Writer::from_path("products.csv")?;
  writer.write_record(["url", "image", "name", "price"])?;

  for product in products {
    writer.write_record([product.url, product.image, product.name, product.price])?;
  }

  writer.flush()?;

  Ok(())
}

fn flatten_text(el: ElementRef<'_>) -> String {
  el
    .text()
    .collect::<Vec<_>>()
    .join("")
    .trim()
    .to_string()
}
