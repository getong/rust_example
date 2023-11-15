// use bytes::Bytes;
// use reqwest::Error;
// // To use stream combinators like `next`
// use prost::Message;
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

    // tokio::pin!(response);

    let mut byte_list = vec![];
    while let Some(chunk) = response.next().await {
        let chunk = chunk?;

        // Assuming `MyProtobufMessage` is the message type you want to decode
        // This part depends on how the server sends the messages

        byte_list.extend_from_slice(&chunk);
    }
    println!("total byte list is {:?}", byte_list);

    let mut cursor = Cursor::new(byte_list);
    match prost::encoding::decode_varint(&mut cursor) {
        Ok(u64_length) => {
            println!("u64_length: {:?}", u64_length);
            match mypackage::MyMessage::decode(cursor) {
                Ok(message) =>
                // Process your message here
                {
                    println!("Received message: {:?}", message)
                }
                _ => println!("can not decode"),
            };
        }

        _ => println!("can not decode"),
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = fetch_and_decode_protobuf_stream("http://localhost:8080/protobuf-stream").await
    {
        eprintln!("Error: {}", e);
    }
}
