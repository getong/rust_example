use std::time::Duration;

use tokio::join;

async fn count_and_wait(n: u64) -> u64 {
  println!("Starting {}", n);
  std::thread::sleep(Duration::from_millis(n * 100));
  println!("Returning {}", n);
  n
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // println!("Hello, world!");

  join!(count_and_wait(1), count_and_wait(2), count_and_wait(3));
  Ok(())
}
