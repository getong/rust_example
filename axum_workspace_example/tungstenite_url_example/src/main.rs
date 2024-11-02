use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing_subscriber;
use url::Url;

#[tokio::main]
async fn main() {
  // Set up logging
  tracing_subscriber::fmt::init();
  println!("Connecting to wss://stream.binance.com:443/ws/btcusdt@miniTicker");

  // WebSocket server address
  let url = "wss://stream.binance.com:443/ws/btcusdt@miniTicker";

  // Connect to the WebSocket server asynchronously
  match connect_async(Url::parse(url).unwrap()).await {
    Ok((mut socket, response)) => {
      println!(
        "Connection successful, HTTP status code: {}",
        response.status()
      );

      // Receive and print messages
      for _ in 0 .. 10 {
        let msg = socket.next().await.expect("Failed to read message");
        match msg {
          Ok(Message::Text(text)) => {
            println!("Received message: {}", text);
          }
          Ok(Message::Ping(ping)) => {
            println!("Received Ping: {:?}", ping);
            // Respond with Pong
            socket
              .send(Message::Pong(ping))
              .await
              .expect("Failed to send Pong");
          }
          Ok(Message::Close(frame)) => {
            println!("Connection closed: {:?}", frame);
            break;
          }
          _ => (),
        }
      }

      // Close the connection
      socket
        .close(None)
        .await
        .expect("Failed to close connection");
    }
    Err(e) => {
      eprintln!("Connection failed: {:?}", e);
    }
  }
}
