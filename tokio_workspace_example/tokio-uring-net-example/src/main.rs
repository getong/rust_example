#[cfg(target_os = "linux")]
use tokio::io::AsyncWriteExt;
#[cfg(target_os = "linux")]
use tokio::net::TcpListener;
#[cfg(target_os = "linux")]
use tokio_uring::fs::File;

#[cfg(target_os = "linux")]
fn main() {
  tokio_uring::start(async {
    // Start a TCP listener
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

    // Accept new sockets
    loop {
      let (mut socket, _) = listener.accept().await.unwrap();

      // Spawn a task to send the file back to the socket
      tokio_uring::spawn(async move {
        // Open the file without blocking
        let file = File::open("hello.txt").await.unwrap();
        let mut buf = vec![0; 16 * 1_024];

        // Track the current position in the file;
        let mut pos = 0;

        loop {
          // Read a chunk
          let (res, b) = file.read_at(buf, pos).await;
          let n = res.unwrap();

          if n == 0 {
            break;
          }

          socket.write_all(&b[..n]).await.unwrap();
          pos += n as u64;

          buf = b;
        }
      });
    }
  });
}

#[cfg(not(target_os = "linux"))]
fn main() {
  // Code for non-Linux systems
  println!("hello world!");
}
