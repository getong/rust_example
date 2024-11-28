use std::{
  io::Write,
  net::{TcpListener, TcpStream},
  thread,
  time::Duration,
};

use rand::Rng;

fn handle_client(label: u32, mut stream: TcpStream) {
  if label == 1 {
    for _ in 0 .. 20 {
      let ran = rand::thread_rng().gen_range(100 .. 200);
      thread::sleep(Duration::from_millis(ran));
      let s = format!("slept for {}", ran);
      stream.write(&s.as_bytes()).unwrap();
    }
  } else if label == 2 {
    for _ in 0 .. 20 {
      let ran = rand::thread_rng().gen_range(150 .. 200);
      thread::sleep(Duration::from_millis(ran));
      let s = format!("slept for {}", ran);
      stream.write(&s.as_bytes()).unwrap();
    }
  } else if label == 3 {
    for _ in 0 .. 20 {
      let ran = rand::thread_rng().gen_range(125 .. 175);
      thread::sleep(Duration::from_millis(ran));
      let s = format!("slept for {}", ran);
      stream.write(&s.as_bytes()).unwrap();
    }
  }
}

fn main() -> std::io::Result<()> {
  println!("Server Started!");
  let server1 = TcpListener::bind("127.0.0.1:8000").unwrap();
  let server2 = TcpListener::bind("127.0.0.1:8001").unwrap();
  let server3 = TcpListener::bind("127.0.0.1:8002").unwrap();

  thread::spawn(move || {
    for stream in server1.incoming() {
      let stream = stream.unwrap();
      thread::spawn(|| {
        handle_client(1, stream);
      });
    }
  });

  thread::spawn(move || {
    for stream in server2.incoming() {
      let stream = stream.unwrap();
      thread::spawn(|| {
        handle_client(2, stream);
      });
    }
  });
  thread::spawn(move || {
    for stream in server3.incoming() {
      let stream = stream.unwrap();
      thread::spawn(|| {
        handle_client(3, stream);
      });
    }
  });
  loop {}
  Ok(())
}
