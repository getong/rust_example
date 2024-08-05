use std::{fs::File, io::Read};

use zip::{read::ZipFile, ZipArchive};

struct ZipFamily;
impl<'a> nolife::Family<'a> for ZipFamily {
  type Family = ZipFile<'a>;
}

async fn zip_file(
  file_name: String,
  member_name: String,
  mut time_capsule: nolife::TimeCapsule<ZipFamily>,
) -> nolife::Never {
  let file = File::open(file_name).unwrap();
  let mut archive = ZipArchive::new(file).unwrap();
  let mut by_name = archive.by_name(&member_name).unwrap();
  time_capsule.freeze_forever(&mut by_name).await
}

struct ZipStreamer {
  zip_scope: nolife::DynBoxScope<ZipFamily>,
}

impl Read for ZipStreamer {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    self.zip_scope.enter(|zip_file| zip_file.read(buf))
  }
}

pub fn zip_streamer(file_name: String, member_name: String) -> impl std::io::Read {
  let zip_scope =
    nolife::DynBoxScope::pin(|time_capsule| zip_file(file_name, member_name, time_capsule));
  ZipStreamer { zip_scope }
}

fn main() {
  let mut output = String::new();
  zip_streamer(
    std::env::args().nth(1).unwrap(),
    std::env::args().nth(2).unwrap(),
  )
  .read_to_string(&mut output)
  .unwrap();
  println!("{}", output);
}

// mkdir toto
// echo "this is titi" > toto/titi.txt
// echo "this is tutu" > toto/tutu.txt
// 7z a toto.zip toto
// cargo run --release -- toto.zip toto/titi.txt  # prints "this is titi"
// cargo run --release -- toto.zip toto/tutu.txt  # prints "this is tutu"
// copy from https://blog.dureuill.net/articles/nolife/
