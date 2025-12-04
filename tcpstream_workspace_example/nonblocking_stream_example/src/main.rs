use std::{
  io::{ErrorKind, Read, Write},
  net::{TcpListener, TcpStream},
  time::Instant,
};

fn main() {
  let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

  println!("Listening on port 7878");

  for stream in listener.incoming() {
    let stream = stream.unwrap();

    handle_connection(stream);
  }
}

fn handle_connection(mut stream: TcpStream) {
  let now = Instant::now();

  // println!("Request:");

  let now2 = Instant::now();

  let _ = stream.set_nonblocking(true);

  let mut buffer = Vec::new();
  let mut chunk = [0u8; 4096];
  loop {
    match stream.read(&mut chunk) {
      Ok(0) => break,
      Ok(n) => buffer.extend_from_slice(&chunk[.. n]),
      Err(ref e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => break,
      Err(e) => {
        eprintln!("read error: {}", e);
        break;
      }
    }
  }

  let _ = stream.set_nonblocking(false);

  let request_text = String::from_utf8_lossy(&buffer);
  println!("{}", request_text);

  println!("now2: {} nanoseconds", now2.elapsed().as_nanos());

  let message = "hello, world";
  let response = format!(
    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: \
     close\r\n\r\n{}",
    message.len(),
    message
  );

  let _ = stream.write_all(response.as_bytes());

  let elapsed = now.elapsed();

  println!(
    "Took {} nanoseconds, {} milliseconds",
    elapsed.as_nanos(),
    elapsed.as_millis()
  );
}
