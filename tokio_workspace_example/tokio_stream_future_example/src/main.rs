use async_stream::stream;
use futures::{future::Future, Stream, StreamExt};

async fn delayed_future(value: i32) -> i32 {
  value
}

// Function that returns a Stream of Futures
fn future_stream() -> impl Stream<Item = impl Future<Output = i32>> {
  stream! {
      for i in 1..=5 {
          // Each item in the stream is a Future
          let future = delayed_future(i);
          yield future;
      }
  }
}

#[tokio::main]
async fn main() {
  let stream = future_stream();
  tokio::pin!(stream);
  while let Some(future_item) = stream.next().await {
    match future_item.await {
      result => println!("Received: {}", result),
    }
  }
}
