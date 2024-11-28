use std::{
  pin::Pin,
  task::{Context, Poll},
};

use tokio_stream::{
  Stream,
  // for next() method
  StreamExt,
};

// A simple stream that emits integers from 0 to 4
struct MyStream {
  count: i32,
}

impl MyStream {
  fn new() -> Self {
    MyStream { count: 0 }
  }
}

impl Stream for MyStream {
  type Item = i32;

  fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
    if self.count < 5 {
      let val = self.count;
      self.count += 1;
      Poll::Ready(Some(val))
    } else {
      Poll::Ready(None)
    }
  }
}

#[tokio::main]
async fn main() {
  let mut stream = MyStream::new();

  while let Some(value) = stream.next().await {
    println!("Got: {}", value);
  }
}
