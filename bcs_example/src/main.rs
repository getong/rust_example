use bcs::{from_bytes, to_bytes};
use serde::Deserialize;

#[derive(Deserialize)]
struct Ip([u8; 4]);

#[derive(Deserialize)]
struct Port(u16);

#[derive(Deserialize)]
struct SocketAddr {
  ip: Ip,
  port: Port,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let some_data: Option<u8> = Some(8);
  assert_eq!(to_bytes(&some_data)?, vec![1, 8]);

  let no_data: Option<u8> = None;
  assert_eq!(to_bytes(&no_data)?, vec![0]);

  let bytes = vec![0x7f, 0x00, 0x00, 0x01, 0x41, 0x1f];
  let socket_addr: SocketAddr = from_bytes(&bytes).unwrap();

  assert_eq!(socket_addr.ip.0, [127, 0, 0, 1]);
  assert_eq!(socket_addr.port.0, 8001);
  Ok(())
}
