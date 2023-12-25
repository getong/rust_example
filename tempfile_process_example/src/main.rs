use std::env;
use std::fs::File;
use std::io::Write;
use std::process::{Command, Stdio};

fn main() {
  // println!("Hello, world!");
  let child_process_code = r#"
    use std::env;
    use std::process;
    use std::thread;
    use std::time::Duration;

    fn main() {
        loop {
            println!("This is the child process speaking!");
            thread::sleep(Duration::from_secs(4));
            let pid = process::id();
            println!("Child process ID: {}", pid);
        }
    }
"#;

  // Create a temporary file
  let mut temp_dir = env::temp_dir();
  temp_dir.push("child_process_code.rs");
  let mut file = File::create(&temp_dir).expect("Failed to create temporary file");

  // Write the Rust code to the temporary file
  file
    .write_all(child_process_code.as_bytes())
    .expect("Failed to write child process code to temporary file");

  let compile_output = Command::new("rustc")
    .arg("-o")
    .arg("/tmp/child_process")
    .arg(&temp_dir)
    .output()
    .expect("Failed to compile child process code");

  if !compile_output.status.success() {
    eprintln!(
      "Error during compilation:\n{}",
      String::from_utf8_lossy(&compile_output.stderr)
    );
    return;
  }

  // Spawn the child process
  let mut child = Command::new("/tmp/child_process")
    .stdout(Stdio::inherit())
    .spawn()
    .expect("Failed to spawn child process");

  println!("Child process spawned with PID: {}", child.id());

  // Wait for the child process to finish
  let status = child.wait().expect("Failed to wait for child process");

  println!("Child process terminated with status: {:?}", status);
}
