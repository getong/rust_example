use std::time::Duration;
use tokio::io::{self, AsyncReadExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

// nc -l 8080

#[tokio::main]
async fn main() {
    // Create a TcpStream and connect to a server
    let stream = TcpStream::connect("127.0.0.1:8080").await.unwrap();

    // Set the read timeout duration
    let timeout_duration = Duration::from_secs(5);

    let mut stream = TimeoutStream::new(stream, timeout_duration);

    loop {
        tokio::select! {
            result = stream.read() => {
                match result {
                    Ok(data) => {
                        // Handle the received data
                        println!("Received data: {}", data);
                    }
                    Err(err) => {
                        // Handle the read error
                        eprintln!("Read error: {}", err);
                    }
                }
            }
            // _ = time::sleep(Duration::from_secs(1)) => {
            //     // Perform some other task every 1 second
            //     println!("Performing another task...");
            // }
        }
    }
}

struct TimeoutStream {
    stream: TcpStream,
    timeout: Duration,
    buffer: Vec<u8>,
}

impl TimeoutStream {
    fn new(stream: TcpStream, timeout: Duration) -> Self {
        let buffer = vec![0u8; 1024];
        Self {
            stream,
            timeout,
            buffer,
        }
    }

    async fn read(&mut self) -> io::Result<String> {
        let read_future = self.stream.read(&mut self.buffer[..]);
        match timeout(self.timeout, read_future).await {
            Ok(result) => {
                let nbytes = result?;
                let data = String::from_utf8_lossy(&self.buffer[..nbytes]).to_string();
                Ok(data)
            }
            Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "Read timed out")),
        }
    }
}
