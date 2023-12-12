use drop_stream::DropStream;
use futures::Stream;

#[tokio::main]
async fn main() {
  // println!("Hello, world!");
  let test_stream = futures::stream::repeat(true);

  let wrapped_stream = DropStream::new(test_stream, move || {
    println!("Stream has been dropped!");
  });
  let mut wrapped_stream = Box::pin(wrapped_stream);
  let waker = futures::task::noop_waker();
  let mut context = futures::task::Context::from_waker(&waker);
  assert_eq!(
    wrapped_stream.as_mut().poll_next(&mut context),
    std::task::Poll::Ready(Some(true))
  );
}
