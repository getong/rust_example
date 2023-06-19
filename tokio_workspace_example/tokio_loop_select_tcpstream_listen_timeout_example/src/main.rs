use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;

const LISTEN_ADDRESS: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() {
    let listen_address: SocketAddr = LISTEN_ADDRESS.parse().expect("Invalid listen address");
    let listener = TcpListener::bind(listen_address)
        .await
        .expect("Failed to bind listener");

    loop {
        if let Ok((client_stream, client_address)) = accept_client(&listener).await {
            println!("Accepted client connection from: {}", client_address);
            tokio::spawn(async move {
                if let Err(err) = handle_client(client_stream).await {
                    eprintln!("Error: {}", err);
                }
            });
        }
    }
}

async fn accept_client(listener: &TcpListener) -> Result<(TcpStream, SocketAddr), io::Error> {
    let (stream, addr) = listener.accept().await?;
    Ok((stream, addr))
}

async fn handle_client(mut client_stream: TcpStream) -> Result<(), io::Error> {
    let mut buffer: Vec<u8> = vec![];

    loop {
        let read_future = client_stream.read(&mut buffer);

        let nbytes = match timeout(Duration::from_secs(5), read_future).await {
            Ok(result) => result?,
            Err(err) => {
                eprintln!("Read error: {}", err);
                return Err(err.into());
            }
        };

        // Process the received data
        let data = String::from_utf8_lossy(&buffer[..nbytes]);
        println!("Received data from client: {}", data);

        // Echo the data back to the client
        if let Err(err) = client_stream.write_all(&buffer[..nbytes]).await {
            eprintln!("Write error: {}", err);
            return Err(err);
        }
    }
}
