use std::sync::Arc;

use tokio::{
  sync::Notify,
  time::{sleep, Duration},
};

#[tokio::main]
async fn main() {
  // Create a Notify object
  // let notify = Notify::new();
  let notify = Arc::new(Notify::new());
  let notify2 = notify.clone();
  // Spawn a task that waits for a notification
  // let notify_clone = notify.clone();
  let waiting_task = tokio::spawn(async move {
    println!("Waiting for notification...");
    notify2.notified().await;
    println!("Notification received!");
  });

  // Sleep for a while to simulate some work
  tokio::spawn(async move {
    sleep(Duration::from_secs(1)).await;
    println!("Sending notification...");
    // Notify the waiting task
    notify.notify_waiters();
  });

  // Wait for the waiting task to finish
  waiting_task.await.expect("Waiting task failed");
}
