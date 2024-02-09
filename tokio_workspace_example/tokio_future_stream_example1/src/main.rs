use async_stream::stream;
use futures::stream::Stream;
use futures::StreamExt;
use tokio::time::{sleep, Duration};

async fn async_stream_generator() -> impl Stream<Item = i32> {
  stream! {
      for i in 0..5 {

          yield i;
      }
  }
}

#[tokio::main]
async fn main() {
  let future_of_stream = async_stream_generator();
  let stream = future_of_stream.await;

  let mut stream = std::pin::pin!(stream);
  let timeout_duration = Duration::from_secs(3);

  loop {
    tokio::select! {
        value = stream.next() => {
            match value {
                Some(v) => println!("Stream value: {}", v),
                None => {
                    println!("Stream ended");
                    break;
                },
            }
        }
        _ = sleep(timeout_duration) => {
            println!("Timeout reached");
            break;
        }
    }
  }
}
