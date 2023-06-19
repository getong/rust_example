// use std::error::Error;
// use std::io::Write;
// use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
// use tokio::net::TcpStream;
// use tokio::time::{sleep, Duration};

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {
//     let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
//     let (reader, mut writer) = stream.split();

//     let mut reader = BufReader::new(reader);
//     let mut buffer = String::new();

//     loop {
//         println!("Enter message: ");
//         let _ = std::io::stdout().flush();

//         buffer.clear();
//         if let Err(e) = reader.read_line(&mut buffer).await {
//             eprintln!("Read error: {}", e);
//             break;
//         }

//         writer.write_all(buffer.as_bytes()).await?;

//         buffer.clear();
//         if let Err(e) = reader.read_line(&mut buffer).await {
//             eprintln!("Read error: {}", e);
//             break;
//         }

//         println!("Received response: {}", buffer);

//         // Delay for 1 second before sending the next message
//         sleep(Duration::from_secs(1)).await;
//     }

//     Ok(())
// }

use std::error::Error;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::sleep;
use std::time::Duration;
use tokio::io::AsyncReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    let (reader, mut writer) = stream.split();

    let mut reader = BufReader::new(reader);

    // Send message to the server
    let message = "Hello, server!";
    writer.write_all(message.as_bytes()).await?;

    // Read the server's response
    let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(vec![]));
    loop {
        let mut locked_buffer = buffer.lock().unwrap();
        tokio::select! {
            _result = reader.read(&mut locked_buffer) => {
                // let nbytes = result?;
                // if nbytes == 0 {
                //     // End of stream, server disconnected
                //     break;
                // }
                // let response = locked_buffer.trim().to_string();
                println!("Server response: {:?}", &locked_buffer[..]);
                locked_buffer.clear();
            }

            _ = sleep(Duration::from_secs(1)) => {
                // let sent_data = locked_buffer.trim().to_string();
                let sent_data = "hello world".to_string();
                writer.write_all(sent_data.as_bytes()).await?;
                println!("Sent data: {}", sent_data);
            }
        }
    }

    // Ok(())
}
