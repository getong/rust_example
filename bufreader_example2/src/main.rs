use std::fs::File;
use std::io::{self, BufRead};

fn main() -> std::result::Result<(), std::io::Error> {
  let filename = "test.bed";
  let fi = File::open(filename)?;
  let mut bedreader = io::BufReader::new(fi); //note here, it is `mut`
  let mut line = String::new(); // not here, it is `mut`
  while bedreader.read_line(&mut line).unwrap() > 0 {
    println!("{}", line.trim_end());
    line.clear();
  }
  Ok(())
}
