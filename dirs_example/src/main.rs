use std::path::PathBuf;

use dirs::home_dir;

fn main() {
  let path_with_tilde = "~/.zshrc";

  let mut absolute_path = PathBuf::new();
  if let Some(home) = home_dir() {
    absolute_path.push(home);
    absolute_path.push(&path_with_tilde[2 ..]); // Skip the tilde (~) and slash (/)
  } else {
    panic!("Failed to get the home directory.");
  }

  println!("Absolute path: {:?}", absolute_path);
}
