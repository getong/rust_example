use once_cell::sync::Lazy;
use rand::Rng;
use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;

pub static REQUEST_CLIENT: Lazy<Client> = Lazy::new(reqwest::Client::new);

const REQUEST_TIMEOUT: u64 = 40;
pub const AUTHORIZATION: &str = "Authorization";

#[tokio::main]
async fn main() {
  let mut rng = rand::thread_rng();

  for i in 0 .. {
    for url in &[
      "https://jsonplaceholder.typicode.com/posts", // Example URL for testing
      "https://jsonplaceholder.typicode.com/comments", // Example URL for testing
      "https://jsonplaceholder.typicode.com/albums", // Example URL for testing
      "https://jsonplaceholder.typicode.com/photos", // Example URL for testing
      "https://jsonplaceholder.typicode.com/todos", // Example URL for testing
      "https://jsonplaceholder.typicode.com/users", // Example URL for testing
      "https://jsonplaceholder.typicode.com/posts/1", // Example URL for testing (specific post)
    ] {
      println!("i is {}, url is {}", i, url);
      let url = *url; // Clone the URL to move into the async block
      tokio::spawn(async move {
        let mut req = REQUEST_CLIENT
          .post(url)
          .timeout(Duration::from_secs(REQUEST_TIMEOUT))
          .header("content-type", "application/json");

        let token = format!("Bearer {}", "your_token_here"); // Replace with real token
        req = req.header(AUTHORIZATION, &token);

        let data = r#"{"key": "value"}"#; // Replace with real data
        if let Err(err) = req.body(data.to_string()).send().await {
          println!("err is {:#?}, url is {}", err, url);
        }
      });

      // Generate a random sleep duration between 1 and 3 seconds
      let sleep_duration = rng.gen_range(1 ..= 3);
      println!("Sleeping for {} seconds", sleep_duration);
      sleep(Duration::from_secs(sleep_duration)).await;
    }
  }
}
