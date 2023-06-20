use std::io;
use std::io::Write;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "127.0.0.1:8080";
    let mut stream = TcpStream::connect(addr).await.unwrap();

    let mut buffer = String::new();
    let mut buf = [0u8; 1024];
    loop {
        _ = io::stdin().read_line(&mut buffer).unwrap();

        _ = stream.write_all(buffer.as_bytes()).await;

        let n = stream
            .read(&mut buf)
            .await
            .expect("failed to read data from socket");

        if n == 0 {
            return Ok(());
        }

        io::stdout().write_all(&buf[..n]).unwrap();
        buffer = String::new();
        buf = [0u8; 1024];
    }
}
