use async_stream::stream;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
  let s = stream! {
      for i in 0..3 {
          yield i;
      }
  };

  // needed for iteration
  tokio::pin!(s);

  while let Some(value) = s.next().await {
    println!("got {}", value);
  }
}
