use futures::future::BoxFuture;
// use std::future::Future;
// use std::pin::Pin;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task;

// Type alias for a boxed future that returns a String and is Sendable
// type BoxedFuture = Pin<Box<dyn Future<Output = String> + Send>>;
type BoxedFuture = BoxFuture<'static, String>;

#[tokio::main]
async fn main() {
  // Create a channel for sending boxed futures
  let (tx, mut rx) = mpsc::channel::<BoxedFuture>(10);

  // Spawn a task to send futures to the channel
  let sender_task = task::spawn(async move {
    for i in 0 .. 5 {
      let future = Box::pin(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        format!("Task {} completed", i)
      });
      tx.send(future).await.expect("Failed to send future");
    }
  });

  // Spawn a task to process futures received from the channel
  let receiver_task = task::spawn(async move {
    while let Some(future) = rx.recv().await {
      let result = future.await;
      println!("{}", result);
    }
  });

  // Wait for both tasks to complete
  let _ = sender_task.await;
  let _ = receiver_task.await;
}
