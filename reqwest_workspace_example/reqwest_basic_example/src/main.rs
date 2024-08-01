// export https_proxy=http://127.0.0.1:7897 http_proxy=http://127.0.0.1:7897 all_proxy=socks5://127.0.0.1:7897

use thiserror::Error;

#[derive(Error, Debug)]
enum MyError {
  #[error("Request error: {0}")]
  Request(#[from] reqwest::Error),
  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),
}

type Result<T> = std::result::Result<T, MyError>;

#[tokio::main]
async fn main() -> Result<()> {
  let body = reqwest::get("https://www.google.com").await?.text().await?;

  println!("body = {body:?}");
  Ok(())
}
