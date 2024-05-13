use tokio::fs;
use walkdir::WalkDir;

const DELETE_DIRECTORY: &str = "/newtec/newtec/temp";

#[tokio::main]
async fn main() {
  // println!("Hello, world!");

  for entry in WalkDir::new(DELETE_DIRECTORY)
    .into_iter()
    .filter_map(|e| e.ok())
  {
    let dir_path = entry.path();
    // println!("{}", dir_path.display());

    if let Err(err) = fs::remove_dir(dir_path).await {
      println!("Error removing directory '{}': {}", dir_path.display(), err);
    }
  }
}
