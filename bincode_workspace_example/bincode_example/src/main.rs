// use std::io::{, Write};
use std::{
  fs::OpenOptions,
  io::{Read, Seek, SeekFrom, Write},
};

use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Entity {
  x: f32,
  y: f32,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct World(Vec<Entity>);

fn main() {
  let mut file = OpenOptions::new()
    .write(true)
    .read(true)
    .create(true)
    .append(true)
    // .truncate(true)
    .open("abc.txt")
    .unwrap();

  // write file
  let world = World(vec![Entity { x: 0.0, y: 4.0 }, Entity { x: 10.0, y: 20.5 }]);
  let encoded: Vec<u8> = bincode::serialize(&world).unwrap();
  println!("encoded:{:?}", encoded);
  file.write_all(&encoded).unwrap();
  _ = file.flush();

  // 8 bytes for the length of the vector, 4 bytes per float.
  assert_eq!(encoded.len(), 8 + 4 * 4);
  let decoded: World = bincode::deserialize(&encoded[..]).unwrap();
  assert_eq!(world, decoded);

  // go back to the beginning of the file
  file.seek(SeekFrom::Start(0)).unwrap();

  // Read the binary data from the file
  let mut buffer: Vec<u8> = Vec::new();
  file.read_to_end(&mut buffer).unwrap();
  println!("buffer: {:?}", buffer);
  // Deserialize the binary data into a MyStruct instance
  let my_struct: World = bincode::deserialize(&buffer).unwrap();
  // Print the deserialized struct
  println!("read from file: {:?}", my_struct);
}
