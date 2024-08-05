use futures::future::BoxFuture;
use tokio::sync::mpsc;

// Type alias for a BoxFuture that outputs a String
type StringFuture = BoxFuture<'static, String>;

// Function to create a simple boxed future
fn create_future(id: u32) -> StringFuture {
  Box::pin(async move { format!("Future {} completed", id) })
}

#[tokio::main]
async fn main() {
  // Create an mpsc channel for transmitting mpsc Senders of BoxFuture
  let (tx, mut rx) = mpsc::channel::<mpsc::Receiver<StringFuture>>(10);

  // Spawn a task to send inner channels (mpsc::Sender<StringFuture>) to the outer channel
  let sender_task = tokio::spawn(async move {
    for i in 0 .. 3 {
      let (inner_tx, inner_rx) = mpsc::channel::<StringFuture>(10);
      tx.send(inner_rx)
        .await
        .expect("Failed to send inner channel");

      // Spawn a task to send futures to the inner channel
      tokio::spawn(async move {
        for j in 0 .. 3 {
          let future = create_future(i * 10 + j);
          inner_tx.send(future).await.expect("Failed to send future");
        }
      });
    }
  });

  // Spawn a task to process inner channels received from the outer channel
  let receiver_task = tokio::spawn(async move {
    while let Some(mut inner_channel) = rx.recv().await {
      // Spawn a task to process futures received from the inner channel
      tokio::spawn(async move {
        while let Some(future) = inner_channel.recv().await {
          let result = future.await;
          println!("{}", result);
        }
      });
    }
  });

  // Wait for both tasks to complete
  let _ = sender_task.await;
  let _ = receiver_task.await;
}
