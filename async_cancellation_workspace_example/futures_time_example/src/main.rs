use futures_time::future::IntoFuture;
// use futures_time::prelude::*;
use futures_time::{task, time::Duration};

async fn wait_until(timer: impl IntoFuture) {
  let timer = timer.into_future();
  timer.await;
}

#[tokio::main]
async fn main() {
  // Wait for a `Duration`.
  wait_until(Duration::from_secs(1)).await;

  // Wait for a concrete future.
  let deadline = task::sleep(Duration::from_secs(1));
  wait_until(deadline).await;
}
