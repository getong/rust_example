use std::{env::set_current_dir, path::Path};

use async_recursion::async_recursion;
use tokio::fs;
use walkdir::WalkDir;

const RENAME_DIRECTORY: &str = "/tmp/a/";

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
        let total_size = WalkDir::new(first_dir_path)
          .min_depth(1)
          .max_depth(3)
          .into_iter()
          .filter_map(|entry| entry.ok())
          .filter_map(|entry| entry.metadata().ok())
          .filter(|metadata| metadata.is_file())
          .fold(0, |acc, m| acc + m.len());
        if total_size == 0 {
          remove_empty_directory(first_dir_path).await
        }
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
