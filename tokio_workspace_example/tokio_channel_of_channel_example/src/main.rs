use tokio::sync::mpsc;
use tokio::task;
// use std::time::Duration;

#[tokio::main]
async fn main() {
  // Create the outer channel
  let (tx, mut rx) = mpsc::channel::<mpsc::Sender<i32>>(10);

  // Spawn a task to create and send inner channels
  let sender_task = task::spawn(async move {
    for i in 0 .. 3 {
      let (inner_tx, mut inner_rx) = mpsc::channel::<i32>(10);

      // Send the inner channel to the outer channel
      tx.send(inner_tx)
        .await
        .expect("Failed to send inner channel");

      // Spawn a task to receive messages from the inner channel
      task::spawn(async move {
        while let Some(message) = inner_rx.recv().await {
          println!("Received on channel {}: {}", i, message);
        }
      });
    }
  });

  // Spawn a task to receive inner channels from the outer channel
  let receiver_task = task::spawn(async move {
    while let Some(inner_channel) = rx.recv().await {
      // Send a message to the inner channel
      inner_channel
        .send(42)
        .await
        .expect("Failed to send message");
    }
  });

  // Wait for both tasks to complete
  let _ = sender_task.await;
  let _ = receiver_task.await;
}
