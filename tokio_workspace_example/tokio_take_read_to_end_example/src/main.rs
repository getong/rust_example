use std::io::{self};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

const BUFFER_SIZE: usize = 1024;

#[tokio::main]
async fn main() -> io::Result<()> {
    // Establish a TCP connection
    let stream = TcpStream::connect("127.0.0.1:1080").await?;
    let mut buf = Vec::with_capacity(BUFFER_SIZE);

    // Read from the TCP stream until the end
    let bytes_read = stream
        .take(BUFFER_SIZE as u64)
        .read_to_end(&mut buf)
        .await?;

    // Truncate the buffer to the actual number of bytes read
    buf.truncate(bytes_read);

    println!("{:?}", buf);

    Ok(())
}
