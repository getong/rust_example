// #[derive(Debug, serde::Serialize)]
#[derive(Debug, Serialize, Deserialize)]
struct DataParse {
  file_num: u16,
  file_name: String,
  file_size: u32,
  file_off: u32,
  file_dum: u8,
}

// use byteorder::BigEndian;
// use byteorder::NativeEndian;
use std::{
  fs::File,
  io::{self, prelude::*, Cursor, Write},
  path::Path,
};

// use bincode::{serialize, deserialize};
use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};

const FILENAME: &str = "abc.txt";

fn read_parse_file(path_of_file: &str) -> io::Result<()> {
  let path = Path::new(&path_of_file);
  let display = path.display();
  println!("path is : {}", &display);

  let mut f = File::open(path)?;

  let mut buffer: Vec<u8> = vec![0; 24];

  f.read_exact(&mut buffer).unwrap();

  let mut vec_pointer = Cursor::new(buffer);

  let filenum = vec_pointer.read_u16::<LittleEndian>().unwrap();
  // println!("filenum:{}", filenum);

  let mut string_buffer = vec![0u8; 13];
  vec_pointer.read_exact(&mut string_buffer).unwrap();
  let filename = String::from_utf8(string_buffer).unwrap();
  // println!("filename:{}" ,filename);
  // match filename.find('\0') {
  //     None => {},
  //     Some(v) =>{
  //         println!("v:{:?}", v);
  //         filename.truncate(v);
  //     },
  // }

  let filesize = vec_pointer.read_u32::<LittleEndian>().unwrap();
  // println!("filenum:{}", filesize);
  let fileoff = vec_pointer.read_u32::<LittleEndian>().unwrap();
  // println!("filenum:{}", fileoff);
  let filendum = vec_pointer.read_u8().unwrap();
  // println!("filenum:{}", filendum);

  let dat_binary = DataParse {
    file_num: filenum,
    file_name: filename,
    file_size: filesize,
    file_off: fileoff,
    file_dum: filendum,
  };

  println!("{:?}", dat_binary);

  Ok(())
}

// #[derive(Debug, serde::Serialize)]
#[derive(Serialize, Deserialize)]
struct MyStruct {
  id: u32,
  name: String,
  age: u8,
}

fn main() {
  // Create an instance of MyStruct
  let my_struct = MyStruct {
    id: 1,
    name: "John Doe".to_string(),
    age: 30,
  };

  // Open a file for writing
  let mut file = File::create("data.bin").unwrap();

  // Write the binary representation of the struct to the file
  file
    .write_all(&bincode::serialize(&my_struct).unwrap())
    .unwrap();

  {
    // Create an instance of MyStruct
    let my_struct = DataParse {
      file_num: 1,
      file_name: "hello".to_string(),
      file_size: 32,
      file_off: 4,
      file_dum: 8,
    };
    // Open a file for writing
    let mut file = File::create(FILENAME).unwrap();
    let bin_buffer = bincode::serialize(&my_struct).unwrap();
    println!("bin_buffer: {}", bin_buffer.len());
    // Write the binary representation of the struct to the file
    file.write_all(&bin_buffer).unwrap();
    _ = file.flush();
  }

  _ = read_parse_file(FILENAME);
}
