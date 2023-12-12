use std::env;
use std::net;

fn listen(socket: &net::UdpSocket, mut buffer: &mut [u8]) -> usize {
  let (number_of_bytes, src_addr) = socket.recv_from(&mut buffer).expect("no data received");

  println!("{:?}", number_of_bytes);
  println!("{:?}", src_addr);

  number_of_bytes
}

fn send(socket: &net::UdpSocket, receiver: &str, msg: &Vec<u8>) -> usize {
  println!("sending data");
  let result = socket
    .send_to(msg, receiver)
    .expect("failed to send message");

  result
}

fn init_host(host: &str) -> net::UdpSocket {
  println!("initializing host");
  let socket = net::UdpSocket::bind(host).expect("failed to bind host socket");

  socket
}

fn main() {
  let host_arg = env::args().nth(1).unwrap();
  let client_arg = env::args().nth(2).unwrap();

  // TODO(alex): Currently hangs on listening, there must be a way to set a timeout, simply
  // setting the timeout to true did not work.
  let mut buf: Vec<u8> = Vec::with_capacity(100);
  let socket = init_host(&host_arg);
  let message = String::from("hello");
  let msg_bytes = message.into_bytes();

  loop {
    while listen(&socket, &mut buf) != 0 {
      println!("boo");
    }
    send(&socket, &client_arg, &msg_bytes);
  }
}
