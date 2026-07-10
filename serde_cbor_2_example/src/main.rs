use std::{error::Error, fs::File};

use serde::{Deserialize, Serialize};
use tempfile::tempdir;

// Types annotated with `Serialize` can be stored as CBOR.
// To be able to load them again add `Deserialize`.
#[derive(Debug, Serialize, Deserialize)]
struct Mascot {
  name: String,
  species: String,
  year_of_birth: u32,
}

fn main() -> Result<(), Box<dyn Error>> {
  let ferris = Mascot {
    name: "Ferris".to_owned(),
    species: "crab".to_owned(),
    year_of_birth: 2015,
  };

  let temp_dir = tempdir()?;
  let ferris_path = temp_dir.path().join("ferris.cbor");
  let ferris_file = File::create(&ferris_path)?;
  // Write Ferris to the given file.
  // Instead of a file you can use any type that implements `io::Write`
  // like a HTTP body, database connection etc.
  serde_cbor_2::to_writer(ferris_file, &ferris)?;

  let ferris_file = File::open(ferris_path)?;
  // Load Ferris from a file.
  // Serde CBOR performs roundtrip serialization meaning that
  // the data will not change in any way.
  let ferris: Mascot = serde_cbor_2::from_reader(ferris_file)?;

  println!("{ferris:?}");
  // prints: Mascot { name: "Ferris", species: "crab", year_of_birth: 2015 }

  Ok(())
}
