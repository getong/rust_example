use std::{fs::File, io, io::Read};

use ouroboros::self_referencing;
use zip::{read::ZipFile, ZipArchive};

#[self_referencing]
struct ZipStreamer {
  archive: ZipArchive<File>,
  #[borrows(mut archive)]
  #[not_covariant]
  reader: ZipFile<'this>,
}

impl Read for ZipStreamer {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    self.with_reader_mut(|reader| reader.read(buf))
  }
}

pub fn zip_streamer(file_name: &str, member_name: &str) -> impl std::io::Read {
  let file = File::open(file_name).unwrap();
  let archive = ZipArchive::new(file).unwrap();
  ZipStreamerBuilder {
    archive,
    reader_builder: |archive| archive.by_name(member_name).unwrap(),
  }
  .build()
}

fn main() {
  println!("Hello, world!");
}
