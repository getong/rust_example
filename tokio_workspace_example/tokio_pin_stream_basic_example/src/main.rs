use async_stream::stream;
use tokio_stream::{Stream, StreamExt};

fn interval_stream(n: usize) -> impl Stream<Item = usize> {
  stream! {
      for i in 0..n {
          yield i;
      }
  }
}

#[tokio::main]
async fn main() {
  // Create two interval streams with different durations
  let stream1 = interval_stream(3);
  let stream2 = interval_stream(5);

  let mut stream1 = std::pin::pin!(stream1);
  let mut stream2 = std::pin::pin!(stream2);

  // Using `select!` to handle values from either stream as they become available
  loop {
    tokio::select! {
        Some(i) = stream1.next() => {
            println!("Tick from stream 1, i is {}", i);
        },
        Some(j) = stream2.next() => {
            println!("Tick from stream 2, j is {}", j);
        },
        else => break,
    }
  }
}
