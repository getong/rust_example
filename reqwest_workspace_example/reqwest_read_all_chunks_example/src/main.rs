// use reqwest::Client;
// use futures_util::StreamExt; // Import StreamExt for using `.next()`
// use tokio::io::{AsyncWriteExt, BufWriter};
// use tokio::fs::File;
// use std::error::Error;

// #[tokio::main]
// async fn main() -> Result<(), Box<dyn Error>> {
//     let url = "https://jsonplaceholder.typicode.com/posts"; // Example API
//     let client = Client::new();

//     let response = client.get(url).send().await?;

//     if response.status().is_success() {
//         let mut body = response.bytes_stream(); // Stream of bytes chunks
//         let mut buffer = Vec::new();

//         // Read all chunks and extend the buffer
//         while let Some(chunk) = body.next().await {
//             let chunk = chunk?; // Handle Result<Bytes, reqwest::Error>
//             buffer.extend_from_slice(&chunk); // Collect bytes into the buffer
//         }

//         // Example: Write the buffer to a file
//         let mut file = BufWriter::new(File::create("output.json").await?);
//         file.write_all(&buffer).await?;
//         file.flush().await?;

//         println!("Data saved to output.json");
//     } else {
//         eprintln!("Request failed with status: {}", response.status());
//     }

//     Ok(())
// }
use reqwest::Client;
use futures_util::StreamExt; // For `.next()`
use std::error::Error;
use std::str;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let url = "https://jsonplaceholder.typicode.com/posts"; // Example API
    let client = Client::new();

    let response = client.get(url).send().await?;

    if response.status().is_success() {
        let mut body = response.bytes_stream(); // Stream of bytes chunks
        let mut buffer = Vec::new();

        // Read all chunks and collect them into the buffer
        while let Some(chunk) = body.next().await {
            let chunk = chunk?; // Handle Result<Bytes, reqwest::Error>
            buffer.extend_from_slice(&chunk); // Collect bytes into the buffer
        }

        // Convert the buffer to a string (assuming UTF-8)
        let result = str::from_utf8(&buffer)?;
        println!("{}", result); // Print the response data to stdout
    } else {
        eprintln!("Request failed with status: {}", response.status());
    }

    Ok(())
}
