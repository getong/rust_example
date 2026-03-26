use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing_subscriber;
use url::Url;

const DEFAULT_URL: &str = "wss://stream.binance.com:443/ws/btcusdt@miniTicker";
const MAX_MESSAGES: usize = 10;

fn parse_websocket_url(raw: &str) -> Result<Url, String> {
  let parsed = Url::parse(raw).map_err(|e| format!("Invalid URL: {e}"))?;
  match parsed.scheme() {
    "ws" | "wss" => Ok(parsed),
    other => Err(format!(
      "Unsupported scheme `{other}`. Only `ws` and `wss` are supported."
    )),
  }
}

fn parse_json_payload(raw: &str) -> Result<Value, String> {
  serde_json::from_str::<Value>(raw).map_err(|e| format!("Invalid JSON payload: {e}"))
}

#[tokio::main]
async fn main() {
  tracing_subscriber::fmt::init();

  // Usage:
  // cargo run -- <ws/wss-url> '<json-payload>'
  // json-payload is optional; if present, it will be sent once after connecting.
  let mut args = std::env::args();
  let _program = args.next();
  let raw_url = args.next().unwrap_or_else(|| DEFAULT_URL.to_owned());
  let raw_json_payload = args.next();

  let url = match parse_websocket_url(&raw_url) {
    Ok(url) => url,
    Err(err) => {
      eprintln!("{err}");
      return;
    }
  };

  println!("Connecting to {url}");

  match connect_async(url).await {
    Ok((mut socket, response)) => {
      println!(
        "Connection successful, HTTP status code: {}",
        response.status()
      );

      if let Some(raw_payload) = raw_json_payload {
        match parse_json_payload(&raw_payload) {
          Ok(json_payload) => {
            let outbound = json_payload.to_string();
            println!("Sending JSON payload: {outbound}");
            if let Err(e) = socket.send(Message::Text(outbound.into())).await {
              eprintln!("Failed to send JSON payload: {e}");
              return;
            }
          }
          Err(err) => {
            eprintln!("{err}");
            return;
          }
        }
      }

      for _ in 0 .. MAX_MESSAGES {
        let msg = match socket.next().await {
          Some(msg) => msg,
          None => {
            println!("WebSocket stream ended");
            break;
          }
        };

        match msg {
          Ok(Message::Text(text)) => match serde_json::from_str::<Value>(&text) {
            Ok(json) => println!(
              "Received JSON:\n{}",
              serde_json::to_string_pretty(&json).unwrap()
            ),
            Err(_) => println!("Received text (non-JSON): {}", text),
          },
          Ok(Message::Binary(binary)) => {
            if let Ok(as_utf8) = std::str::from_utf8(&binary) {
              match serde_json::from_str::<Value>(as_utf8) {
                Ok(json) => println!(
                  "Received JSON (binary UTF-8):\n{}",
                  serde_json::to_string_pretty(&json).unwrap()
                ),
                Err(_) => println!("Received binary UTF-8 (non-JSON): {}", as_utf8),
              }
            } else {
              println!("Received binary data ({} bytes)", binary.len());
            }
          }
          Ok(Message::Ping(ping)) => {
            println!("Received Ping: {:?}", ping);
            if let Err(e) = socket.send(Message::Pong(ping)).await {
              eprintln!("Failed to send Pong: {e}");
              break;
            }
          }
          Ok(Message::Close(frame)) => {
            println!("Connection closed: {:?}", frame);
            break;
          }
          Ok(Message::Pong(pong)) => {
            println!("Received Pong: {:?}", pong);
          }
          Ok(Message::Frame(_)) => {}
          Err(e) => {
            eprintln!("Failed to read message: {e}");
            break;
          }
        }
      }

      if let Err(e) = socket.close(None).await {
        eprintln!("Failed to close connection: {e}");
      }
    }
    Err(e) => {
      eprintln!("Connection failed: {:?}", e);
    }
  }
}
