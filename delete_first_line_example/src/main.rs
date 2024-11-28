// copy from [How to remove the first line of a text file in Rust?](https://stackoverflow.com/questions/62762204/how-to-remove-the-first-line-of-a-text-file-in-rust)
use std::{
  fs::File,
  io::{self, prelude::*},
};

fn remove_first_line(path: &str) -> io::Result<()> {
  let buf = {
    let r = File::open(path)?;
    let mut reader = io::BufReader::new(r);
    reader.read_until(b'\n', &mut Vec::new())?;
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf)?;
    buf
  };
  File::create(path)?.write_all(&buf)?;
  Ok(())
}

fn main() {
  _ = remove_first_line("/tmp/a.txt");
}
