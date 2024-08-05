use std::fs::OpenOptions;
use std::io::Read;
use std::io::Seek;
// use std::io::SeekFrom;
use std::io::Write;

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn main() {
  let mut bs = vec![0; 64 * 1024 * 1024];
  let mut f = OpenOptions::new()
    .write(true)
    .read(true)
    .create(true)
    .open("/tmp/jemallocator_file")
    .unwrap();

  f.write_all(&bs).unwrap();
  f.flush().expect("Failed to flush file");

  // Seek to the beginning of the file
  // f.seek(SeekFrom::Start(0))
  //     .expect("Failed to seek to start of file");
  f.rewind().unwrap();

  let mut ts = 0;
  loop {
    let buf = &mut bs[ts ..];
    let n = f.read(buf).unwrap();
    let n = n as usize;
    if n == 0 {
      break;
    }
    ts += n;
  }

  assert_eq!(ts, 64 * 1024 * 1024);
}
