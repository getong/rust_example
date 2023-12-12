use std::io;
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::tcp::OwnedReadHalf;
use tokio::net::TcpStream;
use tokio_stream::{wrappers::LinesStream, Stream, StreamExt};

// Assume each message is a line
pub struct Message {
  content: String,
}

impl Message {
  pub fn new(content: String) -> Self {
    Message { content }
  }
}

pub fn tcp_stream_into_stream(read_half: OwnedReadHalf) -> impl Stream<Item = io::Result<Message>> {
  let lines = BufReader::new(read_half).lines();
  let lines_stream = LinesStream::new(lines);

  tokio_stream::StreamExt::filter_map(lines_stream, |line_result| match line_result {
    Ok(line) => Some(Ok(Message::new(line))),
    Err(e) => Some(Err(e)),
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
      Ok(message) => println!("Received message: {}", message.content),
      Err(e) => eprintln!("Error reading message: {}", e),
    }
  }
}
