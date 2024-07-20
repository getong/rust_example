use std::path::Path;
use std::process::Command;
use std::{env, fs};

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
                      println!("file_name contpains {:?}", file_name);
                      if file_name.contains("part1") || file_name.contains("part01") {
                        println!("file_name contpains {:?}", file_name);
                        call_unrar_command(file_name);
                      }
                    } else {
                      println!("file_name not contains {:?}", file_name);
                      call_unrar_command(file_name);
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

fn call_unrar_command(file_name: &str) {
  if Command::new("unrar")
    .arg("x")
    .arg(file_name)
    .status()
    .expect("ls command failed to start")
    .success()
  {
    if file_name.contains("part1") || file_name.contains("part01") {
      if Command::new("trash-put")
        .arg(file_name)
        .status()
        .expect("ls command failed to start")
        .success()
      {
        println!("rm {:?}", file_name)
      }
    } else {
      if Command::new("trash-put")
        .arg(file_name)
        .status()
        .expect("ls command failed to start")
        .success()
      {
        println!("rm {:?}", file_name)
      }
    }
  } else {
    println!("unrar {:?} failed", file_name)
  }
}
