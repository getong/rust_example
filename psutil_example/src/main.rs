// use psutil::host::info;
// use psutil::process::{Process, ProcessResult};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Get the current process ID
    // let pid = std::process::id() as u32;

    // // Create a Process object for the current process
    // let process = Process::new(pid)?;

    // Retrieve process information
    match psutil::process::processes() {
        Ok(info_list) => info_list.iter().for_each(|info| match info {
            Ok(info) => {
                println!("Process Name: {:?}", info.name());
                println!(
                    "Process Exe: {:?}",
                    info.exe().unwrap_or_else(|_| String::from("N/A").into())
                );
                println!("Process Arguments: {:?}", info.cmdline());
                println!("Process Parent ID: {:?}", info.parent());
                println!("Process Status: {:?}", info.status());
                // println!(
                //     "Process CPU Usage: {:.2}%",
                //     info.cpu_percent().unwrap_or(0.0)
                // );
                println!("Process Memory Usage: {:?} bytes", info.memory_info());
            }
            Err(_) => {
                println!("error");
            }
        }),
        Err(_) => {
            println!("Process not found or not running");
        }
    }

    Ok(())
}
// the psutil not maintained