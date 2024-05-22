use tokio::fs;
use walkdir::WalkDir;

// TODO: change your directory here
// mkdir -p /tmp/a/b/c/d
// mkdir -p /tmp/a/d
const DELETE_DIRECTORY: &str = "/tmp/a/";
const DELETE_PATH_NAME: &str = "c";

#[tokio::main]
async fn main() {
  for first_entry in WalkDir::new(DELETE_DIRECTORY)
    .min_depth(1)
    .max_depth(1)
    .into_iter()
    .filter_map(|entry| entry.ok())
  {
    let first_dir_path = first_entry.path();
    // println!("first_dir_path: {:?}", first_dir_path);
    if let Ok(first_file_meta) = fs::metadata(first_dir_path).await {
      if first_file_meta.is_dir() {
        // move dirtory
        if let Some(first_file_string) = first_dir_path.to_str() {
          for second_entry in WalkDir::new(first_file_string)
            .min_depth(1)
            .max_depth(1)
            .into_iter()
            .filter_map(|entry| entry.ok())
          {
            if let Some(second_entry_path_str) = second_entry.path().to_str() {
              if second_entry_path_str.contains(DELETE_PATH_NAME) {
                for third_entry in WalkDir::new(second_entry_path_str)
                  .min_depth(1)
                  .max_depth(1)
                  .into_iter()
                  .filter_map(|entry| entry.ok())
                {
                  if let Some(src) = third_entry.path().to_str() {
                    let mut dst = second_entry_path_str.to_string();
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
                if let Err(err) = fs::remove_dir(second_entry_path_str).await {
                  println!(
                    "remove directory {:?} failed, error : {:?}",
                    second_entry_path_str, err
                  );
                }
              }
            }
          }
        }
      }
    }
  }
}
