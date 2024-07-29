use reqwest::Error;
use tokio_stream::StreamExt;

const PROXY_BASE_URL: &str = "http://192.168.80.222:80/";

// deploy

// payg-price
// payg-open
// payg-extend


#[tokio::main]
async fn main() -> Result<(), Error> {
  _ = get_payg_price().await;

  Ok(())
}

async fn get_payg_price() -> Result<(), Error> {
  let client = reqwest::Client::new();

  let payg_price_url = PROXY_BASE_URL.to_owned() + "payg-price";
  let mut stream = client.get(payg_price_url).send().await?.bytes_stream();

  while let Some(item) = stream.next().await {
    if let Ok(item) = item {
      if let Ok(result) = String::from_utf8(item.to_vec()) {
        println!("{}", result);
      }
    }
  }
  Ok(())
}
