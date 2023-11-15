use reqwest::{Client, Error};
use prost::Message;

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

    // Check the response
    if response.status().is_success() {
        let response_text = response.text().await?;
        println!("Response: {}", response_text);
    } else {
        eprintln!("Failed to send POST request.");
    }

    Ok(())
}
