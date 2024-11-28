use futures::future::BoxFuture;
use tokio::{
  sync::mpsc::{self, Sender},
  task,
};
// use std::future::Future;
// use std::pin::Pin;

// Type alias for a BoxFuture that outputs a Sender<String>
type MyBoxFuture = BoxFuture<'static, Sender<String>>;

// Function to create a future that resolves to a Sender<String>
fn create_sender_future() -> MyBoxFuture {
  let (tx, mut rx) = mpsc::channel::<String>(10);
  tokio::spawn(async move {
    while let Some(result_string) = rx.recv().await {
      println!("result_string: {:?}", result_string);
    }
  });
  Box::pin(async { tx })
}

#[tokio::main]
async fn main() {
  // Create an mpsc channel for MyBoxFuture
  let (tx, mut rx) = mpsc::channel::<MyBoxFuture>(10);

  // Spawn a task to send futures to the channel
  let sender_task = task::spawn(async move {
    for _ in 0 .. 5 {
      let future = create_sender_future();
      tx.send(future).await.expect("Failed to send future");
    }
  });

  // Spawn a task to process futures received from the channel
  let receiver_task = task::spawn(async move {
    while let Some(future) = rx.recv().await {
      let sender = future.await;
      sender
        .send("Hello from future!".to_string())
        .await
        .expect("Failed to send message");
    }
  });

  // Wait for both tasks to complete
  let _ = sender_task.await;
  let _ = receiver_task.await;
}
