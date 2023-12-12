use rand::rngs::OsRng;
use rand::Rng;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
  let task1 = tokio::spawn(async {
    let mut rng = OsRng::default();
    let random_number = rng.gen_range(1..=4);
    // Some asynchronous task
    sleep(Duration::from_secs(random_number)).await;
    println!("Task 1 completed");
  });

  let task2 = tokio::spawn(async {
    let mut rng = OsRng::default();
    let random_number = rng.gen_range(1..=4);
    // Some other asynchronous task
    sleep(Duration::from_secs(random_number)).await;
    println!("Task 2 completed");
  });

  tokio::select! {
      _ = task1 => println!("Task 1 joined"),
      _ = task2 => println!("Task 2 joined"),
  }

  println!("Both tasks completed");
}
