use std::error::Error;

use psutil::process::{processes, Process};

fn main() -> Result<(), Box<dyn Error>> {
  // Some psutil process accessors are not implemented on macOS; keep the
  // example limited to the portable ones to avoid panics.
  let process_list = match processes() {
    Ok(list) => list,
    Err(err) => {
      eprintln!("Failed to read processes: {err}");
      eprintln!("Falling back to the current process instead.");
      vec![Process::current()]
    }
  };

  for proc_result in process_list {
    #[allow(unused_mut)]
    let mut process = match proc_result {
      Ok(p) => p,
      Err(err) => {
        eprintln!("Skipping process: {err}");
        continue;
      }
    };

    println!("PID: {}", process.pid());
    match process.name() {
      Ok(name) => println!("Name: {name}"),
      Err(err) => println!("Name: <error: {err}>"),
    }

    #[cfg(target_os = "macos")]
    {
      match process.memory_info() {
        Ok(mem) => println!("Memory (RSS): {} bytes", mem.rss()),
        Err(err) => println!("Memory info: <error: {err}>"),
      }
      match process.cpu_times() {
        Ok(times) => println!(
          "CPU time (user/system): {:?}/{:?}",
          times.user(),
          times.system()
        ),
        Err(err) => println!("CPU times: <error: {err}>"),
      }
    }

    #[cfg(not(target_os = "macos"))]
    {
      match process.exe() {
        Ok(path) => println!("Exe: {}", path.display()),
        Err(err) => println!("Exe: <error: {err}>"),
      }
      match process.cmdline_vec() {
        Ok(Some(args)) => println!("Args: {:?}", args),
        Ok(None) => println!("Args: <none>"),
        Err(err) => println!("Args: <error: {err}>"),
      }
      match process.parent() {
        Ok(Some(parent)) => println!("Parent PID: {}", parent.pid()),
        Ok(None) => println!("Parent PID: <none>"),
        Err(err) => println!("Parent PID: <error: {err}>"),
      }
      match process.status() {
        Ok(status) => println!("Status: {status:?}"),
        Err(err) => println!("Status: <error: {err}>"),
      }
      match process.memory_info() {
        Ok(mem) => println!("Memory (RSS): {} bytes", mem.rss()),
        Err(err) => println!("Memory info: <error: {err}>"),
      }
      match process.cpu_percent() {
        Ok(percent) => println!("CPU: {:.2}%", percent),
        Err(err) => println!("CPU: <error: {err}>"),
      }
    }

    println!();
  }

  Ok(())
}
