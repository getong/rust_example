use std::error::Error;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

async fn handle_connection(mut stream: TcpStream) -> Result<(), Box<dyn Error>> {
    // Handle the connection here
    // For example, you could read incoming data from the stream and send a response
    // back to the client
    let mut buf = [0; 1024];
    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        stream.write_all(&buf[0..n]).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream).await {
                eprintln!("Error handling connection: {}", e);
            }
        });
    }
}
