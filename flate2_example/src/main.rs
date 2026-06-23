use std::io::{Read, Write};

use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};

fn main() -> std::io::Result<()> {
  let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(b"foo")?;
  encoder.write_all(b"bar")?;
  let compressed_bytes = encoder.finish()?;

  let mut decoder = ZlibDecoder::new(compressed_bytes.as_slice());
  let mut decoded = String::new();
  decoder.read_to_string(&mut decoded)?;

  println!("{decoded}");

  Ok(())
}
