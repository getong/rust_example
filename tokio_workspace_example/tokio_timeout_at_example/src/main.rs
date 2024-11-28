use std::time::Duration;

use tokio::{
  runtime::Runtime,
  sync::oneshot,
  time::{timeout_at, Instant},
};

fn main() {
  let rt = Runtime::new().unwrap();

  rt.block_on(async {
    let (_tx, rx) = oneshot::channel::<i32>();

    // Wrap the future with a `Timeout` set to expire 10 milliseconds into the
    // future.
    if let Err(_) = timeout_at(Instant::now() + Duration::from_millis(10), rx).await {
      println!("did not receive value within 10 ms");
    }
  });
}
