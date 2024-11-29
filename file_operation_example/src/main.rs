// use encoding_rs::ASCII;
use std::{fs, fs::File, io::prelude::*, path::Path};

use encoding_rs::UTF_8;

fn read_file() -> std::io::Result<()> {
  let mut file = fs::File::open("test.txt")?;
  let mut contents = String::new();
  file.read_to_string(&mut contents)?;
  println!("{}", contents);
  Ok(())
}

fn write_file() -> std::io::Result<()> {
  let mut file = fs::File::create("test.txt")?;
  file.write_all(b"Hello, world!")?;
  Ok(())
}

fn append_file() -> std::io::Result<()> {
  let mut file = fs::OpenOptions::new().append(true).open("test.txt")?;
  file.write_all(b"Hello, world!")?;
  Ok(())
}

fn write_utf8_content() -> std::io::Result<()> {
  let mut file = fs::File::create("test.txt")?;
  let (cow_string, _, _) = UTF_8.encode("Hello, world!");
  file.write_all(&cow_string.to_owned())?;
  Ok(())
}

// fn write_ascii_content() -> std::io::Result<()> {
//     let mut file = fs::File::create("test.txt")?;
//     file.write_all(ascii.encode("Hello, world!"))?;
//     Ok(())
// }

fn copy_file() -> std::io::Result<()> {
  fs::copy("test.txt", "test_copy.txt")?;
  Ok(())
}

fn rename_file() -> std::io::Result<()> {
  fs::rename("old_name.txt", "new_name.txt")?;
  Ok(())
}

fn remove_file() -> std::io::Result<()> {
  fs::remove_file("test.txt")?;
  Ok(())
}

fn create_dir() -> std::io::Result<()> {
  fs::create_dir("test_dir")?;
  Ok(())
}

fn create_dir_all() -> std::io::Result<()> {
  fs::create_dir_all("path/to/new/dir")?;
  Ok(())
}

fn move_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
  if src.is_dir() {
    fs::create_dir_all(dst)?;
    for entry in src.read_dir()? {
      let entry = entry?;
      let src_path = entry.path();
      let dst_path = dst.join(entry.file_name());
      if src_path.is_dir() {
        move_dir_recursive(&src_path, &dst_path)?;
      } else {
        fs::rename(&src_path, &dst_path)?;
      }
    }
  } else {
    fs::rename(src, dst)?;
  }
  fs::remove_dir_all(src)?;
  Ok(())
}

fn move_all_directory() -> std::io::Result<()> {
  let src = Path::new("old_dir");
  let dst = Path::new("new_dir");
  move_dir_recursive(src, dst)?;
  Ok(())
}

fn remove_dir_all() -> std::io::Result<()> {
  fs::remove_dir_all("test_dir")?;
  Ok(())
}

fn read_to_end() -> std::io::Result<()> {
  {
    let mut file = fs::File::create("example.txt")?;
    _ = file.write_all(b"Hello, world!").unwrap();
  }

  // Open the file
  let file_result = File::open("example.txt");

  let mut file = match file_result {
    Ok(file) => file,
    Err(error) => {
      println!("Failed to open file: {}", error);
      return Err(error);
    }
  };

  // Read file contents
  let mut contents = Vec::new();
  match file.read_to_end(&mut contents) {
    Ok(_) => {
      // Reading successful, do something with the contents
      println!("File contents: {:?}", contents);
      Ok(())
    }
    Err(error) => {
      println!("Failed to read file: {}", error);
      Err(error)
    }
  }
}

fn main() -> std::io::Result<()> {
  _ = read_file();
  _ = write_file();
  _ = append_file();
  _ = write_utf8_content();
  // write_ascii_content()
  _ = copy_file();
  _ = rename_file();
  _ = remove_file();
  _ = create_dir();
  _ = create_dir_all();
  _ = move_all_directory();
  _ = remove_dir_all();
  read_to_end()
}

// copy from https://medium.com/@akaivdo/rust-operating-files-and-folders-7ae4fc3cdad6
