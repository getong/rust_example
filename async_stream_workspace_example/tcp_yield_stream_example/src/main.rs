use async_stream::stream;
use futures::TryStreamExt;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use tokio::{
  io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
  net::TcpListener,
};

#[derive(Debug, thiserror::Error)]
enum Error {
  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),
}

#[tokio::main]
async fn main() -> Result<(), Error> {
  let listener = TcpListener::bind("127.0.0.1:8081").await?;

  println!(
    "Starting server at {}",
    listener.local_addr().map(|x| x.to_string())?
  );

  // let accepts = stream! {
  //   loop {
  //     match listener.accept().await {
  //       Ok(conn) => yield Ok(conn),
  //       Err(e) => {
  //         eprintln!("Failed to accept connection: {}", e);
  //         continue;
  //       },
  //     }
  //   }
  // };
  let accepts = stream! { loop { yield Ok(listener.accept().await?); } };

  accepts
    .try_for_each_concurrent(None, |(stream, addr)| async move {
      if let Err(e) = handle_request(stream, addr).await {
        eprintln!("Error: {}", e);
      }
      Ok(())
    })
    .await
}

async fn handle_request<S>(mut stream: S, _addr: SocketAddr) -> Result<(), Error>
where
  S: AsyncRead + AsyncWrite + Unpin,
{
  let mut buffer = [0; 1024];
  loop {
    stream.read(&mut buffer).await?;
    let request = String::from(String::from_utf8_lossy(&buffer));
    let header = request
      .split("\r\n")
      .map(|line| line.split_at(line.find(": ").unwrap_or(0)))
      .filter(|(_, value)| value.starts_with(": "))
      .map(|(key, value)| (key.to_string(), value[2 ..].to_string()))
      .collect::<BTreeMap<String, String>>();
    let response = format!(
      "<table border=\"1\" cellpadding=\"5\">{}</table>",
      header
        .iter()
        .map(|(key, value)| format!("<tr><td>{key}</td><td>{value}</td></tr>"))
        .collect::<Vec<String>>()
        .concat()
    );
    let response = format!(
      "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}\r\n",
      response.as_bytes().len(),
      response
    );
    stream.write(&response.as_bytes()).await?;
  }
}
// copy from https://www.zhihu.com/question/640220241/answer/3369930196
