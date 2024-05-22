use std::fs;
use std::fs::DirBuilder;
use std::fs::File;
use std::fs::OpenOptions;
use std::os::unix::fs as fsunix;
use std::path::Path;
use std::path::PathBuf;

fn main() {
  println!("Hello, world!");

  // create file
  File::create("./stats.txt");

  // open file
  File::open("stats1.txt").expect("File not found");

  // open file
  OpenOptions::new()
    .write(true)
    .create(true)
    .open("stats2.txt");

  // copy file
  fs::copy("stats1.txt", "stats2.txt").expect("Unable to copy");

  // rename file
  fs::rename("stats1.txt", "stats3.txt").expect("Unable to rename");

  // read file
  let byte_arr = fs::read("stats3.txt").expect("Unable to read file into bytes");
  println!(
    "Value read from file into bytes is {}",
    String::from_utf8(byte_arr).unwrap()
  );

  let string1 = fs::read_to_string("stats3.txt").expect("Unable to read file into string");
  println!("Value read from file into string is {}", string1);

  // write file
  fs::write("stats3.txt", "Rust is exciting,isn't it?").expect("Unable to write to file");

  // metadata
  let file_metadata = fs::metadata("stats.txt").expect("Unable to get file metadata");
  println!(
    "Len: {}, last accessed: {:?}, modified : {:?}, created: {:?}",
    file_metadata.len(),
    file_metadata.accessed(),
    file_metadata.modified(),
    file_metadata.created()
  );

  println!(
    "Is file: {}, Is dir: {}, is Symlink: {}",
    file_metadata.is_file(),
    file_metadata.is_dir(),
    file_metadata.file_type().is_symlink()
  );

  println!("File metadata: {:?}", fs::metadata("stats.txt"));
  println!("Permissions of file are: {:?}", file_metadata.permissions());

  // set permission
  let mut permissions = fs::metadata("stats.txt").unwrap().permissions();
  permissions.set_readonly(true);
  let _ = fs::set_permissions("stats.txt", permissions).expect("Unable to set permission");
  fs::write("stats.txt", "Hello- Can you see me?").expect("Unable to write to file");

  // directory operation
  let dir_entries = fs::read_dir(".").expect("Unable to read directory contents");
  // Read directory contents
  for entry in dir_entries {
    //Get details of each directory entry
    let entry = entry.unwrap();
    let entry_path = entry.path();
    let entry_metadata = entry.metadata().unwrap();
    let entry_file_type = entry.file_type().unwrap();
    let entry_file_name = entry.file_name();
    println!(
      "Path is {:?}.\n Metadata is {:?}\n File_type is {:?}.\n Entry name is{:?}.\n",
      entry_path, entry_metadata, entry_file_type, entry_file_name
    );
  }

  // Get path components
  let new_path = Path::new("/usr/d1/d2/d3/bar.txt");
  println!("Path parent is: {:?}", new_path.parent());
  for component in new_path.components() {
    println!("Path component is: {:?}", component);
  }

  let dir_structure = "/tmp/dir1/dir2/dir3";
  DirBuilder::new()
    .recursive(true)
    .create(dir_structure)
    .unwrap();

  // PathBuf
  let mut f_path = PathBuf::new();
  f_path.push(r"/tmp");
  f_path.push("packt");
  f_path.push("rust");
  f_path.push("book");
  f_path.set_extension("rs");
  // output: Path constructed is "/tmp/packt/rust/book.rs"
  println!("Path constructed is {:?}", f_path);

  // Hard link stats.txt to statsa.txt
  fs::hard_link("stats.txt", "./statsa.txt")?;

  // symlink
  fsunix::symlink("stats.txt", "sym_stats.txt").expect("Cannot create symbolic link");
  let sym_path = fs::read_link("sym_stats.txt").expect("Cannot read link");
  println!("Link is {:?}", sym_path);
}
