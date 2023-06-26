use futures::future::{poll_fn, Future};
use futures::task::{Context, Poll};
use std::pin::Pin;
use std::task::Poll as TaskPoll;
use std::time::Duration;
use tokio::time::sleep;

async fn my_async_operation() -> u32 {
    // Simulate an asynchronous operation that takes some time to complete
    // sleep(Duration::from_secs(2)).await;
    42
}

#[tokio::main]
async fn main() {
    let future = poll_fn(|cx: &mut Context<'_>| -> Poll<u32> {
        // In the closure, perform the polling of the asynchronous operation
        let mut pinned_future: Pin<Box<dyn Future<Output = u32>>> = Box::pin(my_async_operation());
        match Pin::as_mut(&mut pinned_future).poll(cx) {
            TaskPoll::Pending => Poll::Pending,
            TaskPoll::Ready(val) => Poll::Ready(val),
        }
    });

    sleep(Duration::from_secs(2)).await;

    let result = future.await;
    println!("The future completed with result: {:?}", result);
}
