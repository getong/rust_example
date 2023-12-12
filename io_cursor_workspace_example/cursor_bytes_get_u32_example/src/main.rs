use bytes::{Buf, Bytes};
use std::io::Cursor;

fn main() {
  // Sample data: 4 bytes representing a u32 value in little-endian format (e.g., 42)
  let bytes: &[u8] = &[0x2A, 0x00, 0x00, 0x00, 0x00];

  let bytes = Bytes::from(bytes);

  let mut buf = Cursor::new(bytes);

  // if buf.remaining() >= 4 {
  //     let value = buf.get_u32_le();
  //     println!("Read u32 value: {}", value);
  // } else {
  //     eprintln!("Not enough bytes in the buffer to read a u32 value.");
  // }
  println!("{:?}", buf.get_u8());
  println!("{:?}", buf.get_u32());
}
