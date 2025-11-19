use reqwest::get;
use scraper::{Html, Selector};

#[tokio::main]
async fn main() {
  let url = "https://www.scrapingcourse.com/ecommerce/";

  let response = match get(url).await {
    Ok(resp) => resp,
    Err(err) => {
      eprintln!("Request failed for {}: {}", url, err);
      return;
    }
  };

  let html_content = response.text().await.unwrap();
  println!("{}", html_content);

  let document = Html::parse_document(&html_content);
  let product_selector = Selector::parse("li.product").unwrap();
  for product in document.select(&product_selector) {
    let name = product
      .select(&Selector::parse("h2").unwrap())
      .next()
      .map(|e| e.text().collect::<String>());
    let price = product
      .select(&Selector::parse(".price").unwrap())
      .next()
      .map(|e| e.text().collect::<String>());
    let url = product
      .select(&Selector::parse("a").unwrap())
      .next()
      .and_then(|e| e.value().attr("href"))
      .map(|s| s.to_string());
    let image = product
      .select(&Selector::parse("img").unwrap())
      .next()
      .and_then(|e| e.value().attr("src"))
      .map(|s| s.to_string());
    println!(
      "Name: {:?}, Price: {:?}, URL: {:?}, Image: {:?}",
      name, price, url, image
    );
  }
}

// copy from https://thunderbit.com/blog/efficient-rust-web-crawler-using-async-requests/
