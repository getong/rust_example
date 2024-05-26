use std::env::set_current_dir;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use tokio::fs;
use walkdir::WalkDir;

const RENAME_DIRECTORY: &str = "/tmp/a/";
const RENAME_ARGUMENT: &str = "s/abc//g";
const RUN_FLAG: &str = "-n";

#[tokio::main]
async fn main() {
  for first_entry in WalkDir::new(RENAME_DIRECTORY)
    .min_depth(1)
    .max_depth(1)
    .into_iter()
    .filter_map(|entry| entry.ok())
  {
    let first_dir_path = first_entry.path();
    if let Ok(first_file_meta) = fs::metadata(first_dir_path).await {
      if first_file_meta.is_dir() {
        change_dir_and_run_command(first_dir_path).await
      }
    }
  }
}

async fn change_dir_and_run_command(dir_path: &Path) {
  if let Ok(_) = set_current_dir(dir_path) {
    let mut all_files: String = String::new();
    for first_entry in WalkDir::new(dir_path)
      .min_depth(1)
      .max_depth(1)
      .into_iter()
      .filter_map(|entry| entry.ok())
    {
      if let Some(src) = first_entry.path().to_str() {
        all_files.push_str(" ");
        all_files.push_str(src);
      }
    }
    if let Ok(process) = Command::new("/usr/bin/perl-rename")
      .arg(RUN_FLAG)
      .arg(RENAME_ARGUMENT)
      .arg(&all_files)
      .stdout(Stdio::piped())
      .spawn()
    {
      let mut output = String::new();
      match process.stdout.unwrap().read_to_string(&mut output) {
        Err(err) => panic!("couldn't read ps stdout: {}", err),
        Ok(_) => print!("ps output from child process is : \n{}", output),
      }
    }
  }
}
