use walkdir::WalkDir;

fn main() {
  let total_size = WalkDir::new(".")
    .min_depth(1)
    .max_depth(3)
    .into_iter()
    .filter_map(|entry| entry.ok())
    .filter_map(|entry| entry.metadata().ok())
    .filter(|metadata| metadata.is_file())
    .fold(0, |acc, m| acc + m.len());

  println!("Total size: {} bytes.", total_size);

  for entry in WalkDir::new("/Users/gerald/other_project/frontend/src")
    .follow_links(true)
    .into_iter()
    .filter_map(|e| e.ok())
  {
    let f_name = entry.file_name().to_string_lossy();
    if f_name.ends_with(".ts") {
      println!("path: {:?}, {}", entry.path(), f_name);
    }
  }
}
