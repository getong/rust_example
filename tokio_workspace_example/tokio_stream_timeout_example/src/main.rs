use std::time::Duration;

use tokio_stream::StreamExt;

// use tokio::time::timeout;

#[tokio::main]
async fn main() {
  // println!("Hello, world!");
  let int_stream = tokio_stream::iter([1, 2, 3]);

  let int_stream = int_stream.timeout(Duration::from_secs(1));
  // let int_stream = timeout(Duration::from_secs(1), || [1, 2, 3]);
  let mut int_stream = std::pin::pin!(int_stream);

  // When no items time out, we get the 3 elements in succession:
  assert_eq!(int_stream.try_next().await, Ok(Some(1)));
  assert_eq!(int_stream.try_next().await, Ok(Some(2)));
  assert_eq!(int_stream.try_next().await, Ok(Some(3)));
  assert_eq!(int_stream.try_next().await, Ok(None));

  // If the second item times out, we get an error and continue polling the stream:
  // assert_eq!(int_stream.try_next().await, Ok(Some(1)));
  // assert!(int_stream.try_next().await.is_err());
  // assert_eq!(int_stream.try_next().await, Ok(Some(2)));
  // assert_eq!(int_stream.try_next().await, Ok(Some(3)));
  // assert_eq!(int_stream.try_next().await, Ok(None));

  // If we want to stop consuming the source stream the first time an
  // element times out, we can use the `take_while` operator:
  // let mut int_stream = int_stream.take_while(Result::is_ok);

  // assert_eq!(int_stream.try_next().await, Ok(Some(1)));
  // assert_eq!(int_stream.try_next().await, Ok(None));
}
