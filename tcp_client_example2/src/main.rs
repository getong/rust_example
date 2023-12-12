use std::io::prelude::*;
use std::net::TcpStream;
fn main() -> std::io::Result<()> {
  let mut connection = TcpStream::connect("www.baidu.com:80")?;
  connection.write_all(b"GET / HTTP/1.0")?;
  connection.write_all(b"\r\n")?;
  connection.write_all(b"Host: www.baidu.com")?;
  connection.write_all(b"\r\n\r\n")?;
  std::io::copy(&mut connection, &mut std::io::stdout())?;
  Ok(())
}
