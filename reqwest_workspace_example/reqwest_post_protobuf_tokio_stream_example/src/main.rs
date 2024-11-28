use std::io::Cursor;

use bytes::Bytes;
use prost::Message;
use reqwest::{Client, Error};
use tokio_stream::StreamExt;

mod protos;
use protos::*;

#[tokio::main]
async fn main() -> Result<(), Error> {
  let client = Client::new();
  let url = "http://localhost:8080/"; // Replace with your target URL

  let message = mypackage::MyMessage {
    content: "Received your message!".to_string(),
  };

  let bytes = message.encode_to_vec();

  // Perform the POST request
  let response = client
    .post(url)
    .body(bytes)
    .header("Content-Type", "application/protobuf")
    .send()
    .await?;

  let mut byte_list = vec![];

  // Process the response as a stream of bytes
  let mut bytes_stream = response.bytes_stream();
  while let Some(chunk) = bytes_stream.next().await {
    match chunk {
      Ok(bytes) => {
        process_chunk(bytes.clone()).await;
        byte_list.extend_from_slice(&bytes);
      }
      Err(e) => eprintln!("Error while streaming: {}", e),
    }
  }

  println!("total {:?}", &byte_list);
  // Create a Cursor from the collected bytes
  let cursor = Cursor::new(byte_list);

  // Decode the aggregated response using the cursor
  match mypackage::MyMessage::decode(cursor) {
    Ok(decoded_msg) => println!("Decoded message: {:?}", decoded_msg),
    Err(e) => eprintln!("Failed to decode response: {}", e),
  }

  Ok(())
}

async fn process_chunk(chunk: Bytes) {
  // Process each chunk of data here
  println!("Received chunk: {:?}", chunk);
}
