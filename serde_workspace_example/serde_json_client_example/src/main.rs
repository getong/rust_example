use std::error::Error;

use serde_json::to_vec;
use tokio::{io::AsyncWriteExt, net::TcpStream};
mod message;
use message::{GreeRequest, Message}; // Import the Message enum

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let mut socket = TcpStream::connect("127.0.0.1:8080").await?;
  println!("Connected to server!");

  // Create a sample message (choose any of the Message variants)
  let message = Message::GreeRequest(GreeRequest {
    message: String::from("Hello from the client!"),
  });

  // Serialize the message to JSON
  let serialized_message = to_vec(&message)?;

  // Send the serialized message to the server
  socket.write_all(&serialized_message).await?;

  println!("Sent message: {:?}", message);

  Ok(())
}
