use rand::RngExt;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
  let task1 = tokio::spawn(async {
    let random_number: u32 = rand::rng().random_range(1 ..= 4);
    // Some asynchronous task
    sleep(Duration::from_secs(random_number.into())).await;
    println!("Task 1 completed");
  });

  let task2 = tokio::spawn(async {
    let random_number: u32 = rand::rng().random_range(1 ..= 4);
    // Some other asynchronous task
    sleep(Duration::from_secs(random_number.into())).await;
    println!("Task 2 completed");
  });

  tokio::select! {
      _ = task1 => println!("Task 1 joined"),
      _ = task2 => println!("Task 2 joined"),
  }

  println!("Both tasks completed");
}
