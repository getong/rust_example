use bytes::Bytes;
use reqwest::Error;
// To use stream combinators like `next`
use prost::Message;
use std::io::Cursor;
use tokio_stream::StreamExt;
mod protos;
use protos::*;

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub async fn fetch_url(url: &str) -> Result<impl tokio_stream::Stream<Item = Result<Bytes>>> {
    let response = reqwest::get(url).await?;
    Ok(response.bytes_stream())
}

#[tokio::main]
async fn main() {
    let mut byte_list = vec![];
    if let Ok(mut stream) = fetch_url("http://localhost:8080/protobuf-stream").await {
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    println!(
                        "Received bytes: {:?}",
                        String::from_utf8_lossy(&bytes).into_owned()
                    );
                    byte_list.extend_from_slice(&bytes);
                }
                e => eprintln!("Error while streaming: {:?}", e),
            }
        }
    }

    println!("recv {:?}", byte_list);

    let cursor = Cursor::new(byte_list);

    // Decode the aggregated response using the cursor
    match mypackage::MyMessage::decode(cursor) {
        Ok(decoded_msg) => println!("Decoded message: {:?}", decoded_msg),
        Err(e) => eprintln!("Failed to decode response: {}", e),
    }
}
