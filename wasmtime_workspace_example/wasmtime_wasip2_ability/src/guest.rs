use anyhow::Result;
use wstd::{
  http::{Body, BodyExt, Client, Request},
  io::{AsyncRead, AsyncWrite},
  net::TcpStream,
};

pub async fn run() -> Result<()> {
  println!("guest: calling wasi:http outgoing-handler");
  run_wasi_http().await?;

  println!("guest: calling wasi:sockets tcp-create-socket + network");
  run_wasi_sockets().await?;

  println!("guest: wasi-http and wasi-sockets calls completed");
  Ok(())
}

async fn run_wasi_http() -> Result<()> {
  let request = Request::get("http://www.baidu.com/").body(Body::empty())?;
  let response = Client::new().send(request).await?;
  let status = response.status();
  let body = response.into_body().into_boxed_body().collect().await?;

  println!(
    "guest: wasi-http response status={status}, body_bytes={}",
    body.to_bytes().len()
  );
  Ok(())
}

async fn run_wasi_sockets() -> Result<()> {
  let mut stream = TcpStream::connect("baidu.com:80").await?;
  stream
    .write_all(b"HEAD / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
    .await?;
  stream.flush().await?;

  let mut buf = [0_u8; 256];
  let read = stream.read(&mut buf).await?;
  let response_head = String::from_utf8_lossy(&buf[.. read]);
  let status_line = response_head.lines().next().unwrap_or("<empty response>");

  println!("guest: wasi-sockets tcp response: {status_line}");
  Ok(())
}
