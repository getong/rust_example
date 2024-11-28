use serde_json::to_vec;
use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

mod message;
use message::Message; // Import the Message struct

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let mut socket = TcpStream::connect("127.0.0.1:8080").await?;
  println!("Connected to server!");

  // Create a sample message
  let message = Message {
    id: 1,
    content: String::from("Hello, server!"),
  };

  // Serialize the message to JSON
  let serialized_message = to_vec(&message)?;

  // Send the serialized message to the server
  socket.write_all(&serialized_message).await?;

  println!("Sent message: {:?}", message);

  Ok(())
}
