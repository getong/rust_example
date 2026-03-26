use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
  accept_async,
  tungstenite::{Error as WsError, protocol::Message},
};

#[derive(Debug, Parser)]
struct Cli {
  #[arg(long, default_value = "127.0.0.1:9001")]
  listen: String,

  #[arg(long, default_value = "upstream")]
  name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let cli = Cli::parse();
  let listener = TcpListener::bind(&cli.listen).await?;
  println!("{} listening on {}", cli.name, cli.listen);

  loop {
    let (stream, peer_addr) = listener.accept().await?;
    let name = cli.name.clone();
    tokio::spawn(async move {
      if let Err(err) = handle_connection(stream, &name).await {
        eprintln!("{} connection {} error: {}", name, peer_addr, err);
      }
    });
  }
}

async fn handle_connection(
  stream: TcpStream,
  name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let mut ws_stream = match accept_async(stream).await {
    Ok(ws_stream) => ws_stream,
    // Pingora TCP health checks connect without WebSocket handshake.
    Err(WsError::Protocol(_)) => return Ok(()),
    Err(err) => return Err(Box::new(err)),
  };

  while let Some(msg_result) = ws_stream.next().await {
    let msg = msg_result?;

    match msg {
      Message::Text(text) => {
        let reply = format!("[{}] {}", name, text);
        ws_stream.send(Message::Text(reply)).await?;
      }
      Message::Binary(data) => {
        ws_stream.send(Message::Binary(data)).await?;
      }
      Message::Ping(payload) => {
        ws_stream.send(Message::Pong(payload)).await?;
      }
      Message::Pong(_) => {}
      Message::Close(frame) => {
        ws_stream.send(Message::Close(frame)).await?;
        break;
      }
      Message::Frame(_) => {}
    }
  }

  Ok(())
}
