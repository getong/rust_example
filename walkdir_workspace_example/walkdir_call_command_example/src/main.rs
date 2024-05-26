use std::env::set_current_dir;
use std::path::Path;
use std::process::{Command, Stdio};
use tokio::fs;
use walkdir::{DirEntry, WalkDir};

const RENAME_DIRECTORY: &str = "/tmp/a/";
const RENAME_ARGUMENT: &str = "s/abc//g";

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
    Command::new("prel-rename")
      .arg("-n")
      .arg(RENAME_ARGUMENT)
      .arg("*")
      .spawn()
      .expect("perl-rename command failed to start");
  }
}
