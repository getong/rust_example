use std::{pin::Pin, task::Poll, time::Duration};

use futures::Future;
use tokio::{macros::support::poll_fn, time::sleep};

#[tokio::main]
async fn main() {
  let mut sleep1 = sleep(Duration::from_secs(1));
  let mut sleep1 = unsafe { Pin::new_unchecked(&mut sleep1) };

  // this creates a future whose poll method is the closure argument
  poll_fn(|cx| {
    // we poll `sleep1` once, throwing away the result...
    let _ = sleep1.as_mut().poll(cx);
    println!("output from poll_fn");
    // ...and resolve immediately
    Poll::Ready(())
  })
  .await;
}
