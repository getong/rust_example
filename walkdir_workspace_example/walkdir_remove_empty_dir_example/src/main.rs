use std::env::set_current_dir;
use std::path::Path;
use tokio::fs;
use walkdir::WalkDir;
const RENAME_DIRECTORY: &str = "/tmp/a/";
use async_recursion::async_recursion;

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
        remove_empty_directory(first_dir_path).await
      }
    }
  }
}

#[async_recursion]
async fn remove_empty_directory(dir_path: &Path) {
  if let Ok(_) = set_current_dir(dir_path) {
    for first_entry in WalkDir::new(dir_path)
      .min_depth(1)
      .into_iter()
      .filter_map(|entry| entry.ok())
    {
      let first_dir_path = first_entry.path();
      if let Ok(first_file_meta) = fs::metadata(first_dir_path).await {
        if first_file_meta.is_dir() {
          _ = remove_empty_directory(first_dir_path).await;
        }
      }
      _ = fs::remove_dir(first_entry.path()).await;
    }
  }
  _ = fs::remove_dir(dir_path).await;
}
