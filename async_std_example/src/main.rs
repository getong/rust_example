use async_std::{io::prelude::*, net, task};

async fn cheapo_request(host: &str, port: u16, path: &str) -> std::io::Result<String> {
  let mut socket = net::TcpStream::connect((host, port)).await?;
  let request = format!("GET {} HTTP/1.1\r\nHost: {}\r\n\r\n", path, host);

  socket.write_all(request.as_bytes()).await?;
  socket.shutdown(net::Shutdown::Write)?;
  let mut response = String::new();
  socket.read_to_string(&mut response).await?;
  Ok(response)
}

fn main() -> std::io::Result<()> {
  let response = task::block_on(cheapo_request("baidu.com", 80, "/"))?;
  println!("{}", response);
  Ok(())
}
