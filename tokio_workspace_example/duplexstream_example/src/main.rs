use std::error::Error;

use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpStream,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let (mut client, mut server) = tokio::io::duplex(64);

  client.write_all(b"ping").await?;

  let mut buf = [0u8; 4];
  server.read_exact(&mut buf).await?;
  assert_eq!(&buf, b"ping");

  server.write_all(b"pong").await?;

  client.read_exact(&mut buf).await?;
  assert_eq!(&buf, b"pong");

  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
  let addr = listener.local_addr()?;

  tokio::spawn(async move {
    if let Ok((mut server, _)) = listener.accept().await {
      let mut buf = [0; 4];
      if server.read_exact(&mut buf).await.is_ok() && &buf == b"ping" {
        let _ = server.write_all(b"pong").await;
      }
    }
  });

  let mut client = TcpStream::connect(addr).await?;

  client.write_all(b"ping").await?;
  let mut buf = [0; 4];
  client.read_exact(&mut buf).await?;
  assert_eq!(&buf, b"pong");

  println!("Ping pong successful!");

  Ok(()) // Function now returns an Ok result on success
}
