use std::path::Path;
use tokio::fs;
use walkdir::{DirEntry, WalkDir};

// TODO: change your directory here
// mkdir -p /tmp/a/b/c/d
// mkdir -p /tmp/a/d
const DELETE_DIRECTORY: &str = "/tmp/a/";
const DELETE_PATH_NAME: &str = "c";
const ANOTHER_DELETE_PATH_NAME: &str = "d";

#[tokio::main]
async fn main() {
  for first_entry in WalkDir::new(DELETE_DIRECTORY)
    .min_depth(1)
    .max_depth(1)
    .into_iter()
    .filter_map(|entry| entry.ok())
  {
    let first_dir_path = first_entry.path();
    if let Ok(first_file_meta) = fs::metadata(first_dir_path).await {
      if first_file_meta.is_dir() {
        check_to_delete_directory(first_dir_path).await
      }
    }
  }
}

async fn move_files(third_entry: DirEntry, first_file_string: &str) {
  if let Some(src) = third_entry.path().to_str() {
    let mut dst = first_file_string.to_string();
    dst.push_str("/");
    let final_file_name = third_entry.path().file_name().unwrap().to_str().unwrap();
    dst.push_str(final_file_name);
    if let Err(err) = fs::rename(src, &dst).await {
      println!("from : {:?}, to: {:?}, failed, err is :{:?}", src, dst, err);
    }
  } else {
    println!(
      "third_entry.path().to_str() failed, the path is {:?} ",
      third_entry.path()
    );
  }
}

async fn remove_empty_directory(second_entry_path_str: &str) {
  if let Err(err) = fs::remove_dir(second_entry_path_str).await {
    println!(
      "remove directory {:?} failed, error : {:?}",
      second_entry_path_str, err
    );
  }
}

async fn delete_possible_directory(second_entry: DirEntry, first_file_string: &str) {
  if let Some(second_entry_path_str) = second_entry.path().to_str() {
    if second_entry_path_str.contains(DELETE_PATH_NAME)
      || second_entry_path_str.contains(ANOTHER_DELETE_PATH_NAME)
    {
      for third_entry in WalkDir::new(second_entry_path_str)
        .min_depth(1)
        .max_depth(1)
        .into_iter()
        .filter_map(|entry| entry.ok())
      {
        move_files(third_entry, first_file_string).await;
      }
      remove_empty_directory(second_entry_path_str).await;
    }
  }
}

async fn check_to_delete_directory(first_dir_path: &Path) {
  if let Some(first_file_string) = first_dir_path.to_str() {
    for second_entry in WalkDir::new(first_file_string)
      .min_depth(1)
      .max_depth(1)
      .into_iter()
      .filter_map(|entry| entry.ok())
    {
      delete_possible_directory(second_entry, first_file_string).await;
    }
  }
}
