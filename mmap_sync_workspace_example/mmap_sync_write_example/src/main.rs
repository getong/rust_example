use bytecheck::CheckBytes;
use mmap_sync::synchronizer::Synchronizer;
use rkyv::{Archive, Deserialize, Serialize};
use std::time::Duration;

/// Example data-structure shared between writer and reader(s)
#[derive(Archive, Deserialize, Serialize, Debug, PartialEq)]
#[archive_attr(derive(CheckBytes))]
pub struct HelloWorld {
  pub version: u32,
  pub messages: Vec<String>,
}

fn main() {
  // Initialize the Synchronizer
  let mut synchronizer = Synchronizer::new("/tmp/hello_world");

  // Define the data
  let data = HelloWorld {
    version: 7,
    messages: vec!["Hello".to_string(), "World".to_string(), "!".to_string()],
  };

  // Write data to shared memory
  let (written, reset) = synchronizer
    .write(&data, Duration::from_secs(1))
    .expect("failed to write data");

  // Show how many bytes written and whether state was reset
  println!("written: {} bytes | reset: {}", written, reset);
}
