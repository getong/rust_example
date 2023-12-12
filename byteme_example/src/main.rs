use byteme::ByteMe;
pub use num_derive::FromPrimitive;

#[derive(Debug, FromPrimitive)]
pub enum Mode {
  Unavailable = 0,
  Unauthenticated = 1,
  Authenticated = 2,
  Encrypted = 4,
}

#[derive(ByteMe, Debug)]
pub struct FrameOne {
  pub unused: [u8; 12],
  #[byte_me(u32)]
  pub mode: Mode,
  pub challenge: [u8; 16],
  pub salt: [u8; 16],
  pub count: u32,
  pub mbz: [u8; 12],
}

fn main() {
  // println!("Hello, world!");
  let frame = FrameOne {
    unused: [0; 12],
    mode: Mode::Authenticated,
    challenge: [0; 16],
    salt: [0; 16],
    count: 1024,
    mbz: [0; 12],
  };

  let size = FrameOne::SIZE; // Get the number of bytes in the frame
  println!("size: {:?}", size);
  let bytes: Vec<u8> = frame.into(); // Converts the frame into vector of bytes
  println!("bytes: {:?}", bytes);
  let frame: FrameOne = bytes.into(); // Converts the bytes back to frame
  println!("frame: {:?}", frame);
}
