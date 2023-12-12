use std::fs;

use std::io::{self, BufReader};

fn main() -> io::Result<()> {
  let mut f = BufReader::new(fs::File::open("sample.json")?);
  let v: serde_json::Value = serde_json::from_reader(&mut f).unwrap();
  println!("{}", v.is_object());
  return Ok(());
}
