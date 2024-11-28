// use std::process::Command;

use std::{
  io::prelude::*,
  process::{Command, Stdio},
};

fn main() {
  Command::new("ls")
    .arg("-l")
    .arg("-h")
    .spawn()
    .expect("ls command failed to start");

  let status = Command::new("cat")
    .arg("non-existent-file.txt")
    .status()
    .expect("failed to execute cat");

  if status.success() {
    println!("Successful operation");
  } else {
    println!("Unsuccessful operation");
  }

  // Spawn the `ps` command
  let process = match Command::new("ps").stdout(Stdio::piped()).spawn() {
    Err(err) => panic!("couldn't spawn ps: {}", err),
    Ok(process) => process,
  };

  let mut ps_output = String::new();
  match process.stdout.unwrap().read_to_string(&mut ps_output) {
    Err(err) => panic!("couldn't read ps stdout: {}", err),
    Ok(_) => print!("ps output from child process is : \n{}", ps_output),
  }
}
