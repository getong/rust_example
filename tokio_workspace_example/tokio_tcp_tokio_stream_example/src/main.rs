use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::{TcpListener, TcpStream},
};
use tokio_stream::{wrappers::TcpListenerStream, StreamExt};

// telnet localhost 8080

async fn handle_client(mut socket: TcpStream) -> tokio::io::Result<()> {
  let mut buf = [0; 1024];

  loop {
    let n = match socket.read(&mut buf).await {
      Ok(0) => return Ok(()), // connection was closed
      Ok(n) => n,
      Err(e) => {
        eprintln!("failed to read from socket; err = {:?}", e);
        return Err(e);
      }
    };

    if let Err(e) = socket.write_all(&buf[0 .. n]).await {
      eprintln!("failed to write to socket; err = {:?}", e);
      return Err(e);
    }
  }
}

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
  let listener = TcpListener::bind("127.0.0.1:8080").await?;
  let mut incoming = TcpListenerStream::new(listener);

  while let Some(Ok(socket)) = incoming.next().await {
    tokio::spawn(async move {
      if let Err(e) = handle_client(socket).await {
        eprintln!("client handler error: {}", e);
      }
    });
  }

  Ok(())
}
