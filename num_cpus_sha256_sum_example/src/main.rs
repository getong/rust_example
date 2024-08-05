use ring::digest::{Context, Digest, SHA256};
use std::fs::File;
use std::io::{BufReader, Error, Read};
use std::path::Path;
use std::sync::mpsc::channel;
use threadpool::ThreadPool;
use walkdir::WalkDir;

// Verify the iso extension
fn is_iso(entry: &Path) -> bool {
  match entry.extension() {
    Some(e) if e.to_string_lossy().to_lowercase() == "iso" => true,
    _ => false,
  }
}

fn compute_digest<P: AsRef<Path>>(filepath: P) -> Result<(Digest, P), Error> {
  let mut buf_reader = BufReader::new(File::open(&filepath)?);
  let mut context = Context::new(&SHA256);
  let mut buffer = [0; 1024];

  loop {
    let count = buf_reader.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    context.update(&buffer[.. count]);
  }

  Ok((context.finish(), filepath))
}

fn main() -> Result<(), Error> {
  let pool = ThreadPool::new(num_cpus::get());

  let (tx, rx) = channel();

  for entry in WalkDir::new(".")
    .follow_links(true)
    .into_iter()
    .filter_map(|e| e.ok())
    .filter(|e| !e.path().is_dir() && is_iso(e.path()))
  {
    let path = entry.path().to_owned();
    let tx = tx.clone();
    pool.execute(move || {
      let digest = compute_digest(path);
      tx.send(digest).expect("Could not send data!");
    });
  }

  drop(tx);
  for t in rx.iter() {
    let (sha, path) = t?;
    println!("{:?} {:?}", sha, path);
  }
  Ok(())
}
