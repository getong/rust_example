// define a custom data structure
// to store the scraped data
struct Product {
  url: String,
  image: String,
  name: String,
  price: String,
}

fn main() {
  let mut products: Vec<Product> = Vec::new();

  let browser = headless_chrome::Browser::default().unwrap();
  let tab = browser.new_tab().unwrap();
  tab
    .navigate_to("https://www.scrapingcourse.com/ecommerce/")
    .unwrap();

  let html_products = tab.wait_for_elements("li.product").unwrap();

  for html_product in html_products {
    // scraping logic...
    let url = html_product
      .wait_for_element("a")
      .unwrap()
      .get_attributes()
      .unwrap()
      .unwrap()
      .get(1)
      .unwrap()
      .to_owned();
    let image = html_product
      .wait_for_element("img")
      .unwrap()
      .get_attributes()
      .unwrap()
      .unwrap()
      .get(5)
      .unwrap()
      .to_owned();
    let name = html_product
      .wait_for_element("h2")
      .unwrap()
      .get_inner_text()
      .unwrap();
    let price = html_product
      .wait_for_element(".price")
      .unwrap()
      .get_inner_text()
      .unwrap();
    let product = Product {
      url,
      image,
      name,
      price,
    };

    products.push(product);
  }

  // CSV export
  let path = std::path::Path::new("products.csv");
  let mut writer = csv::Writer::from_path(path).unwrap();
  writer
    .write_record(&["url", "image", "name", "price"])
    .unwrap();

  // populate the output file
  for product in products {
    let url = product.url;
    let image = product.image;
    let name = product.name;
    let price = product.price;
    writer.write_record(&[url, image, name, price]).unwrap();
  }

  writer.flush().unwrap();
}


// copy from https://www.zenrows.com/blog/rust-web-scraping#headless-browser-https://www.zenrows.com/blog/rust-web-scraping#headless-browser-scraping
