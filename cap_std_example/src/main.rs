use std::{fs, io};

use cap_std::ambient_authority;
use cap_std::fs::Dir;

/// Open files relative to `dir`.
fn dir_example(dir: &Dir) -> io::Result<()> {
  // This works (assuming symlinks don't lead outside of `dir`).
  let _file = dir.open("the/thing.txt")?;

  // This fails, since `..` leads outside of `dir`.
  assert!(dir.open("../hidden.txt").is_err());

  // This fails, as creating symlinks to absolute paths is not permitted.
  assert!(dir.symlink("/hidden.txt", "misdirection.txt").is_err());

  // However, even if the symlink had succeeded, or, if there is a
  // pre-existing symlink to an absolute directory, following a
  // symlink which would lead outside the sandbox also fails.
  assert!(dir.open("misdirection.txt").is_err());

  Ok(())
}

fn prepare_example_dir() -> io::Result<Dir> {
  fs::create_dir_all("abc")?;

  let dir = Dir::open_ambient_dir("abc", ambient_authority())?;
  dir.create_dir_all("the")?;
  dir.write("the/thing.txt", b"cap-std example\n")?;

  Ok(dir)
}

fn main() -> io::Result<()> {
  let dir = prepare_example_dir()?;
  dir_example(&dir)
}
