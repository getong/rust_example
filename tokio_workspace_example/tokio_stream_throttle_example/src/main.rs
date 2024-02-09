use std::time::Duration;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
  // println!("Hello, world!");

  let item_stream = futures::stream::repeat("one").throttle(Duration::from_secs(2));
  let mut item_stream = std::pin::pin!(item_stream);

  loop {
    // The string will be produced at most every 2 seconds
    println!(
      "current time: {:?}, stream element:{:?}",
      chrono::offset::Local::now(),
      item_stream.next().await
    );
  }
}
