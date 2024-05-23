use walkdir::{DirEntry, WalkDir};

fn is_hidden(entry: &DirEntry) -> bool {
  entry
    .file_name()
    .to_str()
    .map(|s| s.starts_with("."))
    .unwrap_or(false)
}

fn main() {
  let mut it = WalkDir::new("/tmp").into_iter();
  loop {
    let entry = match it.next() {
      None => break,
      Some(Err(err)) => panic!("ERROR: {}", err),
      Some(Ok(entry)) => entry,
    };
    if is_hidden(&entry) {
      if entry.file_type().is_dir() {
        it.skip_current_dir();
      }
      continue;
    }
    println!("{}", entry.path().display());
  }
}
