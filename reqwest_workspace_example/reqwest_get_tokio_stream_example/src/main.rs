// use bytes::Bytes;
// use reqwest::Error;
// // To use stream combinators like `next`
// use prost::Message;
use prost::bytes::Buf;
use std::io::Cursor;
// use tokio_stream::StreamExt;
// mod protos;
// use protos::*;

// pub type Result<T, E = Error> = std::result::Result<T, E>;

// pub async fn fetch_url(url: &str) -> Result<impl tokio_stream::Stream<Item = Result<Bytes>>> {
//     let response = reqwest::get(url).await?;
//     Ok(response.bytes_stream())
// }

// #[tokio::main]
// async fn main() {
//     let mut byte_list = vec![];
//     if let Ok(mut stream) = fetch_url("http://localhost:8080/protobuf-stream").await {
//         while let Some(chunk) = stream.next().await {
//             match chunk {
//                 Ok(bytes) => {
//                     println!(
//                         "Received bytes: {:?}",
//                         String::from_utf8_lossy(&bytes).into_owned()
//                     );
//                     byte_list.extend_from_slice(&bytes);
//                 }
//                 e => eprintln!("Error while streaming: {:?}", e),
//             }
//         }
//     }

//     println!("recv {:?}", byte_list);

//     let cursor = Cursor::new(byte_list);

//     // Decode the aggregated response using the cursor
//     match mypackage::MyMessage::decode(cursor) {
//         Ok(decoded_msg) => println!("Decoded message: {:?}", decoded_msg),
//         Err(e) => eprintln!("Failed to decode response: {}", e),
//     }
// }
// use bytes::Buf;
use prost::Message;
use reqwest::Error;
use tokio_stream::StreamExt;

mod protos;
use protos::*;

async fn fetch_and_decode_protobuf_stream(url: &str) -> Result<(), Error> {
  let client = reqwest::Client::new();

  let mut response = client.get(url).send().await?.bytes_stream();

  // std::pin::pin!(response);

  // let mut byte_list = vec![];
  let mut cursor = Cursor::new(vec![]);
  while let Some(chunk) = response.next().await {
    let chunk = chunk?;
    cursor.get_mut().extend_from_slice(&chunk);

    while cursor.has_remaining() {
      match prost::encoding::decode_varint(&mut cursor) {
        Ok(len) => {
          if cursor.remaining() < len as usize {
            // Not enough data for a complete message, wait for more data
            break;
          }

          let message_end = cursor.position() + len as u64;
          match mypackage::MyMessage::decode(&mut cursor) {
            Ok(message) => println!("Received message: {:?}", message),
            Err(e) => eprintln!("Failed to decode message: {}", e),
          }
          cursor.get_mut().drain(0..message_end as usize);
          cursor.set_position(0);
        }
        Err(_) => {
          // Failed to decode Varint, possibly incomplete data
          break;
        }
      }
    }
  }

  Ok(())
}

#[tokio::main]
async fn main() {
  if let Err(e) = fetch_and_decode_protobuf_stream("http://localhost:8080/protobuf-stream").await {
    eprintln!("Error: {}", e);
  }
}
