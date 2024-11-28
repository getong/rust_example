mod pool;

use std::{
  io,
  io::{Read, Write},
  net::{Shutdown, TcpListener, TcpStream},
};

use pool::ThreadPool;

fn handle_client(mut stream: TcpStream, id: i32) {
  println!("handle stream({}) ...\n", id);
  let mut buffer = [0u8; 512];
  stream.read(&mut buffer).unwrap();

  println!("stream({}) detail in String:", id);
  println!("{}\n", String::from_utf8_lossy(&buffer));
  println!("stream({}) detail in [u8]:", id);
  println!("{:?}\n", buffer);

  let get = b"GET / HTTP/1.1\r\n";
  let (status_line, content) = if buffer.starts_with(get) {
    ("HTTP/1.1 200 OK\r\n\r\n", "Hello World")
  } else {
    ("HTTP/1.1 404 NOT FOUND\r\n\r\n", "Not found")
  };

  let response = format!("{}{}", status_line, content);

  stream.write(response.as_bytes()).unwrap();
  stream.flush().unwrap();

  stream.shutdown(Shutdown::Both).unwrap();

  println!("stream({}) process success.\n", id);
}

fn main() -> io::Result<()> {
  let addr = "127.0.0.1:8080";
  let listener = TcpListener::bind(addr)?;
  let pool = ThreadPool::new(4);

  println!("TcpListener listen at {} ...", addr);

  let mut stream_id = 0;

  for stream in listener.incoming() {
    stream_id += 1;
    let id = stream_id;
    let stream = stream.unwrap();

    println!("incoming new stream, id = {}", id);

    pool.execute(move || {
      handle_client(stream, id);
    });
  }

  Ok(())
}
