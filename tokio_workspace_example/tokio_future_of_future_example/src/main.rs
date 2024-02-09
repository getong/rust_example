// For .fuse()
use futures::future::FutureExt;
use std::future::Future;
use std::pin::Pin;
use tokio::time::{sleep, Duration};

async fn inner_future(value: i32) -> i32 {
  // Simulate some work
  // sleep(Duration::from_secs(2)).await;
  value * 2
}

async fn outer_future() -> Pin<Box<dyn Future<Output = i32>>> {
  // Simulate some delay
  // sleep(Duration::from_secs(1)).await;
  Box::pin(inner_future(5))
}

#[tokio::main]
async fn main() {
  let outer = outer_future().fuse();
  let timeout_duration = Duration::from_secs(3);
  let mut outer = std::pin::pin!(outer);

  loop {
    tokio::select! {
        inner = &mut outer => {
            // Await the inner future
            let result = inner.await;
            println!("Received: {}", result);
        }
        _ = sleep(timeout_duration) => {
            println!("Timeout reached");
        }
    }
  }
}
