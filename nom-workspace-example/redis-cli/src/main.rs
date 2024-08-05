use bytes::{BufMut, BytesMut};
use std::error::Error;
use structopt::StructOpt;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

use tokio::net::TcpStream;

mod commands;
mod reply;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  pretty_env_logger::init();

  let mut stream = TcpStream::connect("127.0.0.1:6379").await?;
  let mut buf = [0u8; 1024];
  let mut resp = BytesMut::with_capacity(1024);

  let (mut reader, mut writer) = stream.split();
  let com = commands::Commands::from_args();
  writer.write(&com.to_bytes()).await?;
  let n = reader.read(&mut buf).await?;
  resp.put(&buf[0 .. n]);
  let reply = reply::Reply::from_resp(&resp);
  println!("{}", reply);
  Ok(())
}
