use prost::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
// use tokio_stream::StreamExt;

mod mypackage {
    include!("mypackage.rs");
}

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("localhost:8080").await.unwrap();

    while let Ok((stream, _addr)) = listener.accept().await {
        tokio::spawn(handle_client(stream));
    }
}

async fn handle_client(mut stream: tokio::net::TcpStream) {
    let mut buf = [0u8; 128]; // Adjust the buffer size based on your message size
    if let Ok(n) = stream.read(&mut buf).await {
        let my_message = match mypackage::MyMessage::decode(&buf[..n]) {
            Ok(message) => message,
            Err(e) => {
                eprintln!("Error decoding message: {:?}", e);
                return;
            }
        };
        println!("Received message: {}", my_message.content);

        // Example response
        let response = mypackage::MyMessage {
            content: "Received your message!".to_string(),
        };

        // Send the response
        let encoded_response = response.encode_to_vec();
        if let Err(e) = stream.write_all(&encoded_response).await {
            eprintln!("Error sending response: {:?}", e);
        }
    }
}
