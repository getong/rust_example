// extern crate mio;
use std::{env, net::SocketAddr};

use mio::{net::TcpListener, Events, Interest, Poll, Token};

// This will be later used to identify the server on the event loop
const SERVER: Token = Token(0);
// Represents a simple TCP server using mio
struct TCPServer {
  address: SocketAddr,
}

// Implementation for the TCP server
impl TCPServer {
  fn new(port: u32) -> Self {
    let address = format!("0.0.0.0:{}", port).parse::<SocketAddr>().unwrap();
    TCPServer { address }
  }
  // Actually binds the server to a given address and runs it
  // This function also sets up the event loop that dispatches
  // events. Later, we use a match on the token on the event
  // to determine if the event is for the server.
  fn run(&mut self) {
    let mut server = TcpListener::bind(self.address).expect("Could not bind to port");
    let mut poll = Poll::new().unwrap();
    poll
      .registry()
      .register(&mut server, SERVER, Interest::READABLE)
      .unwrap();
    let mut events = Events::with_capacity(1024);
    loop {
      poll.poll(&mut events, None).unwrap();
      for event in events.iter() {
        match event.token() {
          SERVER => {
            let (_stream, remote) = server.accept().unwrap();
            println!("Connection from {}", remote);
          }
          _ => {
            unreachable!();
          }
        }
      }
    }
  }
}

fn main() {
  let args: Vec<String> = env::args().collect();
  if args.len() != 2 {
    eprintln!("Please provide only one port number as argument");
    std::process::exit(1);
  }
  let mut server = TCPServer::new(args[1].parse::<u32>().expect("Could not parse as u32"));
  server.run();
}
