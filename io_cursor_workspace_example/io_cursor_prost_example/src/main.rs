use prost::bytes::Bytes;
use prost::Message;
use std::io::{Cursor, Read};

// Define your Protocol Buffers message type
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MyMessage {
  #[prost(int32, tag = "0")]
  pub id: i32,
  #[prost(string, tag = "1")]
  pub content: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  // Simulate network traffic as byte buffers
  let mut network_traffic: Vec<u8> = Vec::new();

  // Serialize the message and append it to the network traffic
  let message = MyMessage {
    id: 1,
    content: "Hello, server!".to_string(),
  };
  let mut buf = Vec::new();
  message.encode_length_delimited(&mut buf)?;
  network_traffic.extend(&buf);

  // Split the network traffic into separate messages
  let mut cursor = Cursor::new(network_traffic.clone());
  loop {
    // Check if there is enough data left in the buffer to read the next message length
    if cursor.position() == network_traffic.len() as u64 {
      break;
    }

    // Get the length of the next message
    let mut message_len_buf = [0; 4];
    cursor.read_exact(&mut message_len_buf)?;
    let message_len = u32::from_le_bytes(message_len_buf) as usize;

    // Create a buffer to read the next message into
    let mut message_buf = vec![0; message_len];
    cursor.read_exact(&mut message_buf)?;

    // Decode the next message
    let message = MyMessage::decode_length_delimited(Bytes::from(message_buf))?;

    // Handle the message
    println!("Received message: {:?}", message);
  }

  Ok(())
}
