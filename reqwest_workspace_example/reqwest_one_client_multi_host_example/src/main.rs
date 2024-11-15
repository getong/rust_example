use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

pub static REQUEST_CLIENT: Lazy<Client> = Lazy::new(reqwest::Client::new);

const REQUEST_TIMEOUT: u64 = 40;

#[tokio::main]
async fn main() {
  let mut rng = rand::thread_rng();

  for i in 0 .. 9999 {
    for url in &[
      "https://www.baidu.com",
      "https://www.qq.com",
      "https://www.163.com",
    ] {
      println!("i is {}, url is {}", i, url);
      let req = REQUEST_CLIENT
        .get(*url)
        .timeout(Duration::from_secs(REQUEST_TIMEOUT));

      if let Err(err) = req.send().await {
        println!("err is {:#?}, url is {}", err, url);
      }

      // Generate a random sleep duration between 1 and 5 seconds
      let sleep_duration = rng.gen_range(1 ..= 5);
      println!("Sleeping for {} seconds", sleep_duration);
      sleep(Duration::from_secs(sleep_duration)).await;
    }
  }
}
