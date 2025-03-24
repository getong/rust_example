use std::{
  error::Error,
  net::{IpAddr, Ipv4Addr, SocketAddr},
  sync::Arc,
};

use quinn::{ClientConfig, Connection, Endpoint, RecvStream, SendStream};
use rustls::RootCertStore;
use rustls_pki_types::{CertificateDer, pem::PemObject};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8090);

  let client_config = create_client_config()?;
  let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
  endpoint.set_default_client_config(client_config);

  let conn = connect_to_server(&mut endpoint, server_addr).await?;
  println!("Connected to QUIC server at {}\n", server_addr);

  let (mut send, mut recv) = conn.open_bi().await?;

  send_message(&mut send, "Hello, I am the client").await?;
  println!("You sent: Hello, I am the client.");

  handle_response(&mut recv).await?;

  Ok(())
}

fn create_client_config() -> Result<ClientConfig, Box<dyn Error>> {
  let mut root_cert_store = RootCertStore::empty();
  let cert = CertificateDer::from_pem_file("/tmp/quinn-chat-certs/cert.pem")
    .expect("please `cargo run quinn_chat_recgen` first");
  root_cert_store.add(cert)?;

  Ok(ClientConfig::with_root_certificates(Arc::new(
    root_cert_store,
  ))?)
}

async fn connect_to_server(
  endpoint: &mut Endpoint,
  server_addr: SocketAddr,
) -> Result<Connection, Box<dyn Error>> {
  Ok(endpoint.connect(server_addr, "localhost")?.await?)
}

async fn send_message(send: &mut SendStream, message: &str) -> Result<(), Box<dyn Error>> {
  send.write(message.as_bytes()).await?;
  send.finish()?;
  send.stopped().await?;
  Ok(())
}

async fn handle_response(recv: &mut RecvStream) -> Result<(), Box<dyn Error>> {
  let mut buf = vec![0; 1024];
  match recv.read(&mut buf).await {
    Ok(_size) => {
      println!("Server responded: {}", String::from_utf8_lossy(&buf));
      Ok(())
    }
    Err(e) => {
      eprintln!("Error reading from stream: {}", e);
      Err(Box::new(e))
    }
  }
}
