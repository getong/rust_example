use std::error::Error;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // Connect to a TCP server
  let mut stream = TcpStream::connect("127.0.0.1:8080").await?;

  // Split the stream into read and write halves
  let (mut read_half, _) = stream.split();

  // Create a buffer to hold the read data
  let mut buffer = vec![0u8; 1024];

  // Read data from the stream
  let mut total_bytes = 0;
  loop {
    let num_bytes = read_half.read(&mut buffer[total_bytes ..]).await?;
    if num_bytes == 0 {
      // Reached end of stream
      break;
    }
    total_bytes += num_bytes;

    if total_bytes == buffer.len() {
      // Resize buffer to accommodate more data
      buffer.resize(total_bytes * 2, 0);
    }
  }

  // Process the read data
  let data = &buffer[.. total_bytes];
  println!("Received data: {:?}", data);

  Ok(())
}
