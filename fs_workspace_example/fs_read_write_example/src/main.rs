use std::{
  fs::File,
  io::{BufRead, BufReader, Error, Write},
};

fn main() -> Result<(), Error> {
  let path = "lines.txt";

  let mut output = File::create(path)?;
  write!(output, "Rust\n💖\nFun")?;

  let input = File::open(path)?;
  let buffered = BufReader::new(input);

  for line in buffered.lines() {
    println!("{}", line?);
  }

  Ok(())
}
