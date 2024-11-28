// https://teukka.tech/peer-discovery.html
use std::{
  net::{Ipv4Addr, SocketAddrV4, UdpSocket},
  thread,
};

use anyhow::Result;

static MULTI_CAST_ADDR: Ipv4Addr = Ipv4Addr::new(224, 0, 0, 1);

pub fn listen() -> Result<(), anyhow::Error> {
  let socket_address: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 9778);
  let bind_addr = Ipv4Addr::new(0, 0, 0, 0);
  let socket = UdpSocket::bind(socket_address)?;
  println!("Listening on: {}", socket.local_addr().unwrap());
  socket.join_multicast_v4(&MULTI_CAST_ADDR, &bind_addr)?;
  Ok(())
}

pub fn cast() -> Result<()> {
  let socket_address: SocketAddrV4 = SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0);
  let socket = UdpSocket::bind(socket_address)?;
  socket.connect(SocketAddrV4::new(MULTI_CAST_ADDR, 9778))?;
  // Don't send messages to yourself.
  // In this case self discovery is for human developers, not machines.
  socket.set_multicast_loop_v4(false)?;
  let data = String::from("{\"username\": \"test\"}");
  loop {
    socket.send(data.as_bytes())?;
    thread::sleep(std::time::Duration::from_secs(2));
  }
  // Ok(())
}
fn main() {
  thread::spawn(|| {
    _ = listen();
  });
  _ = cast();
}
