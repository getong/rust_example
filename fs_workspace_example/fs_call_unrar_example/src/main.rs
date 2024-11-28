use std::{env, fs, path::Path, process::Command};

use regex::Regex;

const UNRAR_DIRECTORY: &str = "UNRAR_DIRECTORY";

#[tokio::main]
async fn main() {
  if let Ok(directory) = dotenv::var(UNRAR_DIRECTORY) {
    // println!("{:?}", directory);

    let path = Path::new(&directory);
    match env::set_current_dir(&path) {
      Ok(_) => {
        // println!(
        //   "Successfully changed working directory to {}",
        //   path.display()
        // );
        match fs::read_dir(path) {
          Ok(entries) => {
            for entry in entries {
              if let Ok(entry) = entry {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("rar") {
                  if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
                    // println!("{}", file_name);
                    if file_name.contains("part") {
                      // println!("file_name contpains {:?}", file_name);
                      if file_name.contains("part1") || file_name.contains("part01") {
                        // println!("file_name contpains {:?}", file_name);
                        if call_unrar_command(file_name) {
                          let pattern1 = Regex::new(r"part\d+.*\.rar$").unwrap();

                          // Replace the pattern with an empty string
                          let mut result_string = pattern1.replace(file_name, "").to_string();
                          // println!("Modified: {}", result_string);
                          result_string.push_str(r"part\d+.*\.rar$");
                          let pattern2 = Regex::new(&result_string).unwrap();

                          delete_all_files(&directory, pattern2);
                        }
                      }
                    } else {
                      // println!("file_name not contains {:?}", file_name);
                      if call_unrar_command(file_name) {
                        call_trash_command(file_name);
                      }
                    }
                  }
                }
              }
            }
          }
          Err(e) => eprintln!("Error reading directory: {}", e),
        }
      }
      Err(e) => eprintln!("Error changing working directory: {}", e),
    }
  }
}

fn call_unrar_command(file_name: &str) -> bool {
  Command::new("unrar")
    .arg("x")
    .arg(file_name)
    .status()
    .expect("unrar command failed to start")
    .success()
}

fn call_trash_command(file_name: &str) {
  println!("delete {:?}", file_name);
  if Command::new("trash-put")
    .arg(file_name)
    .status()
    .expect("trash-put command failed to start")
    .success()
  {
    println!("rm {:?}", file_name)
  }
}

fn delete_all_files(directory: &str, pattern2: Regex) {
  println!("pattern2: {:?}", pattern2);
  let path = Path::new(directory);
  match env::set_current_dir(&path) {
    Ok(_) => match fs::read_dir(path) {
      Ok(entries) => {
        for entry in entries {
          if let Ok(entry) = entry {
            let path = entry.path();
            if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
              if pattern2.is_match(file_name) {
                // println!("{}", file_name);
                call_trash_command(file_name);
              }
            }
          }
        }
      }
      _ => println!("error"),
    },
    _ => println!("error"),
  }
}
