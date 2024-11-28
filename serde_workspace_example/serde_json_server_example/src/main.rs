use std::error::Error;

use serde_json::de::from_slice;
use tokio::{io::AsyncReadExt, net::TcpListener};
mod message;
use message::Message;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let listener = TcpListener::bind("127.0.0.1:8080").await?;
  println!("Server running on 127.0.0.1:8080");

  loop {
    let (mut socket, _) = listener.accept().await?;
    println!("New connection established!");

    tokio::spawn(async move {
      let mut buf = vec![0; 1024];

      loop {
        // Read from the socket
        match socket.read(&mut buf).await {
          Ok(0) => break, // Connection was closed
          Ok(n) => {
            // Parse the received JSON into the Message enum
            match from_slice::<Message>(&buf[.. n]) {
              Ok(message) => match message {
                Message::GreeRequest(req) => {
                  println!("Received GreeRequest: {:?}", req);
                }
                Message::GreetResponse(res) => {
                  println!("Received GreetResponse: {:?}", res);
                }
                Message::AnotherMessage(msg) => {
                  println!("Received AnotherMessage: {:?}", msg);
                }
                Message::FourthMessage(msg) => {
                  println!("Received FourthMessage: {:?}", msg);
                }
              },
              Err(e) => {
                println!("Failed to deserialize message: {}", e);
              }
            }
          }
          Err(e) => {
            eprintln!("Failed to read from socket: {}", e);
            break;
          }
        }
      }
    });
  }
}
