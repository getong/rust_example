use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};

use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bind the server to a TCP socket
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server listening on 127.0.0.1:8080");

    loop {
        // Accept a new client connection
        let (socket, _) = listener.accept().await?;

        // Spawn a new task to handle the client
        tokio::spawn(async move {
            if let Err(e) = handle_client(socket).await {
                eprintln!("Error handling client: {}", e);
            }
        });
    }
}

async fn handle_client(mut socket: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    // Create a new file for writing
    let mut file = File::create("received_file.txt").await?;

    let mut buffer = [0; 1024];
    loop {
        // Receive data from the client
        let bytes_read = socket.read(&mut buffer).await?;

        if bytes_read == 0 {
            // Reached the end of the file transfer
            break;
        }

        // Write the received data to the file
        file.write_all(&buffer[..bytes_read]).await?;
    }

    println!("File transfer completed successfully");
    Ok(())
}
