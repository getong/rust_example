use std::error::Error;

use tokio::{
  io::{AsyncReadExt, AsyncWriteExt},
  net::TcpListener,
  runtime::Runtime,
};

fn main() -> Result<(), Box<dyn Error>> {
  // Create the runtime
  let rt = Runtime::new()?;

  // Spawn the root task
  rt.block_on(async {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
      let (mut socket, _) = listener.accept().await?;

      tokio::spawn(async move {
        let mut buf = [0; 1024];

        // In a loop, read data from the socket and write the data back.
        loop {
          let n = match socket.read(&mut buf).await {
            // socket closed
            Ok(0) => return,
            Ok(n) => n,
            Err(e) => {
              println!("failed to read from socket; err = {:?}", e);
              return;
            }
          };

          // Write the data back
          if let Err(e) = socket.write_all(&buf[0 .. n]).await {
            println!("failed to write to socket; err = {:?}", e);
            return;
          }
        }
      });
    }
  })
}
