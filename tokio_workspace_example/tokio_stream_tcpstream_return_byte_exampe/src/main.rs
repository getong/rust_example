use bytes::Bytes;
use futures::stream::unfold;
use std::io;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpStream;
use tokio_stream::{Stream, StreamExt};

pub fn tcp_stream_into_stream(read_half: OwnedReadHalf) -> impl Stream<Item = io::Result<Bytes>> {
  unfold(read_half, |mut read_half| async {
    let mut buffer = vec![0; 1024]; // Adjust the buffer size as needed
    match read_half.read(&mut buffer).await {
      Ok(0) => None, // End of stream
      Ok(n) => {
        // Resize buffer to the actual number of bytes read
        buffer.truncate(n);
        Some((Ok(Bytes::from(buffer)), read_half))
      }
      Err(e) => Some((Err(e), read_half)),
    }
  })
}

// nc -l 3000

#[tokio::main]
async fn main() {
  let tcp_stream = TcpStream::connect("127.0.0.1:3000").await.unwrap();
  let (read_half, mut write_half) = tcp_stream.into_split();
  write_half.write(r"hello world".as_bytes()).await.unwrap();

  let message_stream = tcp_stream_into_stream(read_half);
  tokio::pin!(message_stream);
  while let Some(message_result) = message_stream.next().await {
    match message_result {
      Ok(message) => println!("Received message: {:?}", message),
      Err(e) => eprintln!("Error reading message: {}", e),
    }
  }
}
