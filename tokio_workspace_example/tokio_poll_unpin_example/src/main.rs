use futures::FutureExt;
use std::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};
use tokio::time::Duration;
use tokio::time::Sleep;

#[tokio::main]
async fn main() {
  let fut = MyFuture::new();
  println!("Awaiting fut...");
  fut.await;
  println!("Awaiting fut... done!");
}

struct MyFuture {
  sleep: Pin<Box<Sleep>>,
}

impl MyFuture {
  fn new() -> Self {
    Self {
      sleep: Box::pin(tokio::time::sleep(Duration::from_secs(1))),
    }
  }
}

impl Future for MyFuture {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
    println!("MyFuture::poll()");
    // let sleep = Pin::new(&mut self.sleep);
    // let sleep = self.sleep.as_mut();
    // sleep.poll(cx)
    let sleep = &mut self.sleep;
    sleep.poll_unpin(cx)
  }
}

// copy from https://fasterthanli.me/articles/pin-and-suffering
