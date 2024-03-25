use std::net::UdpSocket;

fn handle_udp_client(socket: UdpSocket) {
  let mut buffer = [0; 512];
  let (_, client_address) = socket.recv_from(&mut buffer).unwrap();
  let request = String::from_utf8_lossy(&buffer);
  println!("Received UDP request from {}: {}", client_address, request);
  let response = b"Hello, UDP!";
  socket.send_to(response, client_address).unwrap();
}

// echo -n “Test message” | nc -u 127.0.0.1 8080
fn main() {
  let socket = UdpSocket::bind("127.0.0.1:8080").unwrap();
  loop {
    handle_udp_client(socket.try_clone().unwrap());
  }
}
