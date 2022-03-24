use futures::FutureExt;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::time::Duration;
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
        self.sleep.poll_unpin(cx)
    }
}
