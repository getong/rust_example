use futures::FutureExt;
use tokio_stream::{Stream, StreamExt};

fn outer() -> impl Stream<Item = i32> {
  inner().into_stream()
}

async fn inner() -> i32 {
  42
}

#[tokio::main]
async fn main() {
  let out_stream = outer();
  let mut out_stream = std::pin::pin!(out_stream);
  while let Some(i) = out_stream.next().await {
    println!("i: {:?}", i);
  }
}
