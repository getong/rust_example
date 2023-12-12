use async_stream::try_stream;
use std::io::Cursor;
use tokio::io::{self, AsyncReadExt};
use tokio_stream::Stream;
use tokio_stream::StreamExt;

// Function that simulates async data reading and returns a Stream of Vec<u8>
pub fn read_stream<R>(mut reader: R) -> impl Stream<Item = io::Result<Vec<u8>>>
where
  R: AsyncReadExt + Unpin + Send + 'static,
{
  try_stream! {
      let mut buffer = [0u8; 1024]; // Buffer size

      loop {
          let n = reader.read(&mut buffer).await?; // Async read into buffer
          if n == 0 {
              break; // End of stream
          }
          yield buffer[..n].to_vec(); // Yield a Vec<u8> of the read bytes
      }
  }
}

#[tokio::main]
async fn main() -> io::Result<()> {
  // Example: Using a Cursor as an async reader.
  // In a real-world application, this could be a file, network stream, etc.
  let data = b"Hello, world! This is a test data stream.";
  let reader = Cursor::new(data);

  // Create the stream
  let stream = read_stream(reader);
  tokio::pin!(stream);
  // Consume the stream
  while let Some(chunk) = stream.next().await {
    println!("Received chunk: {:?}", chunk?);
  }

  Ok(())
}
