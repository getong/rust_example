use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{self, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
//use tokio::task;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to the server
    let socket = Arc::new(Mutex::new(TcpStream::connect("127.0.0.1:8080").await?));
    println!("Connected to server");

    loop {
        // Prompt the user for a file path
        println!("Enter file path (or 'exit' to quit):");
        let mut input = String::new();
        io::BufReader::new(io::stdin())
            .read_line(&mut input)
            .await?;

        let file_path = input.trim().to_owned(); // Clone the file path

        if file_path == "exit" {
            // Exit the loop and end the program
            break;
        }

        // Clone the shared socket for the closure
        let shared_socket = Arc::clone(&socket);

        // Spawn a new task to handle the file transfer
        tokio::spawn(async move {
            let mut socket = shared_socket.lock().await;
            if let Err(e) = send_file(&mut socket, &file_path).await {
                eprintln!("Error sending file: {}", e);
            }
        });
    }

    Ok(())
}

async fn send_file(
    socket: &mut TcpStream,
    file_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Open the file for reading
    let mut file = File::open(file_path).await?;

    let mut buffer = [0; 1024];
    loop {
        // Read data from the file
        let bytes_read = file.read(&mut buffer).await?;

        if bytes_read == 0 {
            // Reached the end of the file
            break;
        }

        // Send the data to the server
        socket.write_all(&buffer[..bytes_read]).await?;
    }

    println!("File transfer completed successfully");
    Ok(())
}
