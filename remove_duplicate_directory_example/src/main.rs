// use error_chain::error_chain;
use std::fs;
use std::io::Error;
use std::path::Path;
use std::path::PathBuf;

// when TWO_DIRECTORY_FLAG is true
// mkdir -p /tmp/a/b/c/d
// touch /tmp/a/b/c/d/file.txt

// when TWO_DIRECTORY_FLAG is false
// mkdir -p /tmp/a/b/c
// touch /tmp/a/b/c/file.txt
const TWO_DIRECTORY_FLAG: bool = false;
// change the directory you want to modify
const FILE_PATH: &[u8] = b"/tmp/a/b/";

const WRITE_FLAG: bool = false;

// error_chain! {
//     foreign_links {
//         Io(std::io::Error);
//         SystemTimeError(std::time::SystemTimeError);
//     }
// }

fn main() -> Result<(), Error> {
  let dir_name =
    String::from_utf8(tilde_expand::tilde_expand(FILE_PATH)).expect("Invalid UTF-8 sequence");

  let current_dir = PathBuf::from(dir_name);

  for entry in fs::read_dir(current_dir)? {
    let first_entry = entry?;
    let first_path = first_entry.path();

    let first_metadata = fs::metadata(&first_path)?;

    if first_metadata.is_dir() {
      // println!("path : {:?}", path);
      if TWO_DIRECTORY_FLAG {
        for second_entry in fs::read_dir(first_path)? {
          let second_entry = second_entry?;
          let second_path = second_entry.path();

          let second_metadata = fs::metadata(&second_path)?;

          if second_metadata.is_dir() {
            _ = check_path(second_path);
          }
        }
      } else {
        _ = check_path(first_path);
      }
    }
  }

  Ok(())
}

fn check_path(first_path: PathBuf) -> Result<(), Error> {
  let sub_paths = fs::read_dir(first_path.clone())?;

  let mut sub_dir_count = 0;
  let mut sub_dir_name: PathBuf = PathBuf::new();
  for sub_entry in sub_paths {
    let sub_entry = sub_entry?;
    let sub_path = sub_entry.path();
    let sub_metadata = fs::metadata(&sub_path)?;
    if sub_metadata.is_dir() {
      sub_dir_count += 1;
      sub_dir_name = sub_path.clone();
    }
  }
  if sub_dir_count == 1 {
    // println!(
    //     "path {} has only one  dirs, {}",
    //     sub_path.display(),
    //     sub_dir_name.display()
    // );
    move_directory(&sub_dir_name, &first_path)?
  }
  Ok(())
}

fn move_directory(source: &PathBuf, destination: &Path) -> std::io::Result<()> {
  // Read the entries in the source directory
  if WRITE_FLAG {
    for entry in fs::read_dir(source)? {
      let entry = entry?;
      let entry_path = entry.path();

      let new_path = destination.join(entry.file_name());
      fs::rename(&entry_path, &new_path)?;
    }
    fs::remove_dir(source)?;
  }
  println!(
    "source: {:?}, destination: {:?}",
    source.display(),
    destination.display()
  );

  Ok(())
}
