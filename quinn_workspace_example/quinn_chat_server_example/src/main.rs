use std::{error::Error, net::SocketAddr};

use quinn::{Connection, Endpoint, Incoming, SendStream, ServerConfig};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let addr: SocketAddr = "127.0.0.1:8090".parse()?;

  let cert = CertificateDer::from_pem_file("/tmp/quinn-chat-certs/cert.pem")
    .expect("please `cargo run quinn_chat_recgen` first");
  let private_key = PrivateKeyDer::from_pem_file("/tmp/quinn-chat-certs/cert.key")
    .expect("please `cargo run quinn_chat_recgen` first");

  let server_config = ServerConfig::with_single_cert(vec![cert], private_key)?;

  let endpoint = Endpoint::server(server_config, addr)?;
  println!("QUIC server running at {}", addr);

  while let Some(conn) = endpoint.accept().await {
    tokio::spawn(async move {
      handle_connection(conn).await;
    });
  }

  Ok(())
}

async fn handle_connection(conn: Incoming) {
  match conn.await {
    Ok(new_conn) => {
      println!("\n(New connection from {:?})", new_conn.remote_address());
      if let Err(e) = handle_stream(new_conn).await {
        eprintln!("Error handling stream: {}", e);
      }
    }
    Err(e) => eprintln!("Connection error: {:?}", e),
  }
}

async fn handle_stream(conn: Connection) -> Result<(), Box<dyn std::error::Error>> {
  match conn.accept_bi().await {
    Ok((mut send, mut recv)) => {
      let mut buf = vec![0; 1024];
      match recv.read(&mut buf).await {
        Ok(_size) => {
          println!("Received: {}", String::from_utf8_lossy(&buf));
          let response = "Hello, I am the server";
          send_response(&mut send, response).await?;
          println!("Responding: {}", response);
        }
        Err(e) => {
          eprintln!("Error reading from connection: {}", e);
        }
      }
    }
    Err(e) => {
      eprintln!("Error accepting connection: {}", e);
    }
  }

  Ok(())
}

async fn send_response(
  send: &mut SendStream,
  response: &str,
) -> Result<(), Box<dyn std::error::Error>> {
  send.write(response.as_bytes()).await?;
  send.finish()?;
  send.stopped().await?;
  Ok(())
}

// copy from https://github.com/Matheus-git/quic-chat
