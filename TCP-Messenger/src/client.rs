use std::{
  io::{Read, Write},
  net::TcpStream,
};

#[derive(Debug)]
pub struct Client {
  pub stream: TcpStream,
}

impl Client {
  pub fn new(stream: TcpStream) -> Self {
    Client { stream: stream }
  }

  pub fn read_stream(&mut self) -> Vec<u8> {
    let mut buf: Vec<u8> = vec![0; 250];
    let stream = &mut self.stream;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_millis(300)));
    let result = stream.read_exact(&mut buf);
    match result {
      Ok(_) => {}
      Err(_) => {
        eprintln!("Failed to read from the client stream")
      }
    }
    buf
  }

  pub fn write_stream(&mut self, message: &str) {
    if message.len() == 0 {
      return;
    }
    let stream = &mut self.stream;
    let result = stream.write(message.as_bytes());

    match result {
      Ok(_) => eprintln!("Written successfully"),
      _ => eprintln!("couldnt connect to ip:port"),
    }
  }
}
