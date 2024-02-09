use async_stream::stream;
use std::pin::Pin;
use tokio_stream::{Stream, StreamExt};

// Function to create an inner stream
fn inner_stream(start: i32, end: i32) -> impl Stream<Item = i32> {
  stream! {
      for i in start..end {
          yield i;
      }
  }
}

// Function to create an outer stream of streams
fn outer_stream() -> impl Stream<Item = Pin<Box<dyn Stream<Item = i32> + Send>>> {
  stream! {
      for i in 0..3 {
          // Each item in the outer stream is an inner stream
          let inner = inner_stream(i * 10, (i + 1) * 10);
          yield Box::pin(inner) as Pin<Box<dyn Stream<Item = i32> + Send>>;
      }
  }
}

#[tokio::main]
async fn main() {
  let outer = outer_stream();
  let mut outer = std::pin::pin!(outer);
  while let Some(inner) = outer.next().await {
    println!("New inner stream:");
    let mut inner = std::pin::pin!(inner);
    while let Some(value) = inner.next().await {
      println!("Value: {}", value);
    }
  }
}
