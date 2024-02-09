// For .fuse()
use futures::future::FutureExt;
// use futures::pin_mut; // For pin_mut!
use std::time::Duration;
use tokio::time::sleep;

// Example of an asynchronous function
async fn async_function1() -> String {
  sleep(Duration::from_secs(1)).await;
  "Function 1 completed".to_string()
}

// Another asynchronous function
async fn async_function2() -> String {
  sleep(Duration::from_secs(2)).await;
  "Function 2 completed".to_string()
}

#[tokio::main]
async fn main() {
  // Create futures from the async functions and fuse them
  let future1 = async_function1().fuse();
  let future2 = async_function2().fuse();

  // Pin the futures
  // pin_mut!(future1, future2);
  let mut future1 = std::pin::pin!(future1);
  let mut future2 = std::pin::pin!(future2);

  // Using `select!` to wait for the first future to complete
  loop {
    tokio::select! {
        result = &mut future1 => {
            println!("Future 1: {}", result);
            // break;
        },
        result = &mut future2 => {
            println!("Future 2: {}", result);
            // break;
        },
    }
  }
}
