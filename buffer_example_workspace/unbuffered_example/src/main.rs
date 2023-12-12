use std::fs;
use std::io::{self, Write};

fn main() -> io::Result<()> {
  let mut f = fs::File::create("/tmp/unbuffered.txt")?;
  f.write(b"foo")?;
  f.write(b"\n")?;
  f.write(b"bar\nbaz\n")?;
  return Ok(());
}
