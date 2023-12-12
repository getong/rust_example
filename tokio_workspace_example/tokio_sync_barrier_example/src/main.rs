use std::sync::Arc;
use tokio::sync::Barrier;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
  // Create a Barrier with a count of 3
  let barrier = Arc::new(Barrier::new(3));

  // Spawn three tasks that will synchronize at the barrier
  for i in 0..3 {
    let barrier_clone = barrier.clone();
    tokio::spawn(async move {
      println!("Task {} is working...", i);
      // Simulate some work
      sleep(Duration::from_secs(1)).await;
      println!(
        "Task {} has finished its work and is waiting at the barrier.",
        i
      );
      // Wait at the barrier
      barrier_clone.wait().await;
      println!("Task {} has passed the barrier.", i);
    });
  }

  // Sleep for a while to allow tasks to complete
  sleep(Duration::from_secs(2)).await;
}
