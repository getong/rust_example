// copy from [Find a file in current or parent directories](https://codereview.stackexchange.com/questions/236743/find-a-file-in-current-or-parent-directories)

use std::env;
use std::path::{Path, PathBuf};

const RUSV_FILENAME: &str = "Cargo.toml";

/**
 * Find a rusv file in the current or parent directories of the given directory.
 */

fn find_rusv_file(starting_directory: &Path) -> Option<PathBuf> {
  let mut path: PathBuf = starting_directory.into();
  let file = Path::new(RUSV_FILENAME);

  loop {
    path.push(file);

    if path.is_file() {
      break Some(path);
    }

    if !(path.pop() && path.pop()) {
      // remove file && remove parent
      break None;
    }
  }
}

fn main() -> std::io::Result<()> {
  let path = env::current_dir()?;

  match find_rusv_file(&path) {
    Some(filepath) => println!("Rusv file was found: {:?}", filepath),
    None => println!("No rusv file was found."),
  };

  Ok(())
}
