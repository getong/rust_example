use prost::Message;
use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

mod mypackage {
  include!("mypackage.rs");
}

// nc -l 8080
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  // let message = mypackage::MyMessage {
  //     content: "hello".to_string(),
  // };
  let message = mypackage::MyMessage {
    content: "Received your message!".to_string(),
  };

  let address = "localhost:8080"; // Replace with the server's address
  let mut stream = TcpStream::connect(address).await?;

  // Serialize the message and send it over the TCP connection
  let bytes = message.encode_to_vec();
  stream.write_all(&bytes).await?;

  tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
  Ok(())
}
