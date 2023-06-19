use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::sleep;

const LISTEN_ADDRESS: &str = "127.0.0.1:8080";
const RECONNECT_DELAY_SECONDS: u64 = 2;

#[tokio::main]
async fn main() {
    let listen_address: SocketAddr = LISTEN_ADDRESS.parse().expect("Invalid listen address");
    let listener = Arc::new(Mutex::new(
        TcpListener::bind(listen_address)
            .await
            .expect("Failed to bind listener"),
    ));

    loop {
        match accept_client(Arc::clone(&listener)).await {
            Ok((mut client_stream, client_address)) => {
                println!("Accepted client connection from: {}", client_address);

                if let Err(err) = handle_client(&mut client_stream).await {
                    eprintln!("Error: {}", err);
                }

                println!("Client connection closed: {}", client_address);
            }
            Err(err) => {
                eprintln!("Accept error: {}", err);
            }
        }

        // Delay before accepting new connections
        sleep(Duration::from_secs(RECONNECT_DELAY_SECONDS)).await;
    }
}

async fn accept_client(
    listener: Arc<Mutex<TcpListener>>,
) -> Result<(TcpStream, SocketAddr), io::Error> {
    let listener = listener.lock().unwrap();
    listener.accept().await.map(|(stream, addr)| (stream, addr))
}

async fn handle_client(client_stream: &mut TcpStream) -> Result<(), io::Error> {
    let mut buffer = [0u8; 1024];

    loop {
        let nbytes = match client_stream.read(&mut buffer).await {
            Ok(nbytes) if nbytes == 0 => {
                // End of stream, client disconnected
                return Ok(());
            }
            Ok(nbytes) => nbytes,
            Err(err) => {
                // Read error, return it to the caller
                return Err(err);
            }
        };

        // Process the received data
        let data = String::from_utf8_lossy(&buffer[..nbytes]);
        println!("Received data from client: {}", data);

        // Echo the data back to the client
        client_stream.write_all(&buffer[..nbytes]).await?;
    }
}
