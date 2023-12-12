use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
  // println!("Hello, world!");

  let handle = tokio::spawn(async {
    sleep(Duration::from_secs(1)).await;
    println!("2");
  });

  println!("1");
  drop(handle); // Drop the task handle.
  sleep(Duration::from_secs(2)).await;
  println!("done");
}
