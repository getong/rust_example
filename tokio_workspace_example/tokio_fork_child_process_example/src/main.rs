// use tokio::prelude::*;
// use nix::sys::wait;
use nix::sys::wait::wait;
use nix::unistd::{fork, ForkResult};
use std::io::Error;
use std::io::ErrorKind;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
// use wait::wait;

// Limit to 1 thread
#[tokio::main(worker_threads = 1)]
async fn main() -> Result<(), Error> {
    let mut socks = UnixStream::pair()?;

    match unsafe { fork() } {
        Ok(ForkResult::Parent { .. }) => {
            eprintln!("Writing!");
            socks.0.write_u32(31337).await?;
            eprintln!("Written!");
            wait().unwrap();
            Ok(())
        }

        Ok(ForkResult::Child) => {
            eprintln!("Reading from master");
            let msg = socks.1.read_u32().await?;
            eprintln!("Read from master {}", msg);
            Ok(())
        }

        Err(_) => Err(Error::new(ErrorKind::Other, "oh no!")),
    }
}
