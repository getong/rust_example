use std::{
  fs::File,
  io::{BufReader, Read, Write},
};

use data_encoding::HEXUPPER;
use error_chain::error_chain;
use ring::digest::{Context, Digest, SHA256};

error_chain! {
    foreign_links {
        Io(std::io::Error);
        Decode(data_encoding::DecodeError);
    }
}

fn sha256_digest<R: Read>(mut reader: R) -> Result<Digest> {
  let mut context = Context::new(&SHA256);
  let mut buffer = [0; 1024];

  loop {
    let count = reader.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    context.update(&buffer[.. count]);
  }

  Ok(context.finish())
}

fn main() -> Result<()> {
  let path = "file.txt";

  let mut output = File::create(path)?;
  write!(output, "We will generate a digest of this text")?;

  let input = File::open(path)?;
  let reader = BufReader::new(input);
  let digest = sha256_digest(reader)?;

  println!("SHA-256 digest is {}", HEXUPPER.encode(digest.as_ref()));

  Ok(())
}

// copy from https://rust-lang-nursery.github.io/rust-cookbook/cryptography/hashing.html#calculate-the-sha-256-digest-of-a-file
