use bytecheck::CheckBytes;
use mmap_sync::synchronizer::Synchronizer;
use rkyv::{Archive, Deserialize, Serialize};

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

  // Read data from shared memory
  let data = unsafe { synchronizer.read::<HelloWorld>() }.expect("failed to read data");

  // Access fields of the struct
  println!("version: {} | messages: {:?}", data.version, data.messages);
}
