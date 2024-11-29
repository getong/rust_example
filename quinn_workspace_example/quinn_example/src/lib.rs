#![cfg(feature = "rustls")]

use std::{
  fs::File,
  io::{BufReader, Error},
  net::SocketAddr,
  str,
  sync::Arc,
};

use bytes::{BufMut, Bytes, BytesMut};
use futures_util::StreamExt;
use protobuf::Message;
pub use quinn;
use quinn::{ClientConfig, Endpoint, Incoming, IncomingUniStreams, NewConnection, ServerConfig};
use rustls;
use sodiumoxide::crypto::secretbox::Key;
use tokio::{self, sync::mpsc};

use crate::{anyhow::anyhow, internal_config::RTT_LEN, time::Instant, ResultType};

const SERVER_NAME: &str = "xremote.autox.tech";
const CHANNEL_LATENCY: i64 = 1_000;
const DROP_MSG: bool = true;

pub(crate) struct Cert<'a> {
  ca_file: &'a str,
  client_cert_file: &'a str,
  client_key_file: &'a str,
  server_cert_file: &'a str,
  server_key_file: &'a str,
}

#[cfg(not(debug_assertions))]
lazy_static::lazy_static! {
    static ref CERT: Cert<'static> = Cert {
        ca_file: "/etc/ssl/ca.cert",
        client_cert_file: "/etc/ssl/client.cert",
        client_key_file: "/etc/ssl/client.key",
        server_cert_file: "/etc/ssl/server.fullchain",
        server_key_file: "/etc/ssl/server.rsa",
    };
}

const MAX_BUFFER_SIZE: usize = 128;
type Value = Vec<u8>;
type Sender = mpsc::Sender<(Instant, Value)>;
type Receiver = mpsc::Receiver<(Instant, Value)>;

pub struct Connection {
  pub conn: quinn::Connection,
  pub endpoint: Option<Endpoint>,
  self_sender: Sender,
  out_receiver: Receiver,
}

impl Connection {
  pub async fn new_for_client(
    server_addr: SocketAddr,
    local_addr: SocketAddr,
    ms_timeout: u64,
  ) -> ResultType<Self> {
    let client_cfg = client::config(CERT.ca_file, CERT.client_cert_file, CERT.client_key_file);
    let mut endpoint = Endpoint::client(local_addr).expect("create client endpoint");
    endpoint.set_default_client_config(client_cfg);

    let connecting = super::timeout(
      ms_timeout,
      endpoint
        .connect(server_addr, SERVER_NAME)
        .expect("connect to server error"),
    )
    .await??;

    let NewConnection {
      connection,
      uni_streams,
      ..
    } = connecting;

    let (self_sender, self_receiver) = mpsc::channel::<(Instant, Value)>(MAX_BUFFER_SIZE);
    let (out_sender, out_receiver) = mpsc::channel::<(Instant, Value)>(MAX_BUFFER_SIZE);
    tokio::spawn(process_stream(
      connection.clone(),
      uni_streams,
      out_sender,
      self_receiver,
      connection.remote_address(),
    ));

    Ok(Connection {
      conn: connection,
      endpoint: Some(endpoint),
      self_sender,
      out_receiver,
    })
  }

  pub async fn new_for_server(conn: quinn::Connecting) -> ResultType<Self> {
    let quinn::NewConnection {
      connection,
      uni_streams,
      ..
    } = conn.await?;

    let (self_sender, self_receiver) = mpsc::channel::<(Instant, Value)>(MAX_BUFFER_SIZE);
    let (out_sender, out_receiver) = mpsc::channel::<(Instant, Value)>(MAX_BUFFER_SIZE);
    tokio::spawn(process_stream(
      connection.clone(),
      uni_streams,
      out_sender,
      self_receiver,
      connection.remote_address(),
    ));

    Ok(Connection {
      conn: connection,
      endpoint: None,
      self_sender,
      out_receiver,
    })
  }

  #[inline]
  pub async fn next(&mut self) -> Option<Result<BytesMut, Error>> {
    match self.out_receiver.recv().await {
      None => None,
      Some((_, req_bytes)) => {
        let mut bytes = BytesMut::new();
        bytes.put_slice(&req_bytes);
        return Some(Ok(bytes));
      }
    }
  }

  #[inline]
  pub async fn next_timeout(&mut self, ms: u64) -> Option<Result<BytesMut, Error>> {
    if let Ok(res) = tokio::time::timeout(std::time::Duration::from_millis(ms), self.next()).await {
      res
    } else {
      None
    }
  }

  #[inline]
  pub async fn send(&mut self, msg: &impl Message) -> ResultType<()> {
    self.send_raw(msg.write_to_bytes()?).await
  }

  #[inline]
  pub async fn send_raw(&mut self, msg: Vec<u8>) -> ResultType<()> {
    self
      .self_sender
      .send((Instant::now(), msg))
      .await
      .map_err(|e| anyhow!("failed to shutdown stream: {}", e))
  }

  pub async fn send_bytes(&mut self, bytes: Bytes) -> ResultType<()> {
    self.send_raw(bytes.to_vec()).await?;
    Ok(())
  }

  #[inline]
  pub fn remote_address(&self) -> SocketAddr {
    self.conn.remote_address()
  }

  #[inline]
  pub fn local_address(&self) -> Option<SocketAddr> {
    if let Some(endpoint) = &self.endpoint {
      return Some(endpoint.local_addr().expect("get local address error"));
    }
    None
  }

  #[inline]
  pub async fn shutdown(&self) -> std::io::Result<()> {
    self.conn.close(0u32.into(), b"done");
    // Give the peer a fair chance to receive the close packet
    if let Some(endpoint) = &self.endpoint {
      endpoint.wait_idle().await;
    }
    Ok(())
  }

  pub fn set_raw(&mut self) {}

  pub fn set_key(&mut self, _key: Key) {}
}

async fn process_stream(
  conn: quinn::Connection,
  mut uni_streams: IncomingUniStreams,
  out_sender: Sender,
  mut self_receiver: Receiver,
  addr: SocketAddr,
) {
  let a = async move {
    loop {
      match self_receiver.recv().await {
        Some((instant, msg)) => {
          let latency = instant.elapsed().as_millis() as i64;
          if DROP_MSG && latency as i64 > CHANNEL_LATENCY && msg.len() != RTT_LEN {
            log::debug!(
              "The duration of the message in the quic sending queue is: {:?}",
              latency
            );
            continue;
          }

          if let Ok(mut sender_stream) = conn.open_uni().await {
            log::debug!("send {} bytes to stream", msg.len());
            match sender_stream.write_all(&msg).await {
              Err(e) => {
                log::error!("send msg error: {:?}", e);
              }
              _ => {}
            }
          }
        }
        None => break,
      }
    }
    log::info!("exit send loop");
    Err::<(), ()>(())
  };

  let b = async move {
    loop {
      match uni_streams.next().await {
        Some(result) => match result {
          Err(quinn::ConnectionError::ApplicationClosed { .. }) => {
            log::info!("connection terminated by peer {:?}.", &addr);
            break;
          }
          Err(err) => {
            log::info!("read msg for peer {:?} with error: {:?}", &addr, err);
            break;
          }
          Ok(recv_stream) => {
            if let Ok(bytes) = recv_stream.read_to_end(usize::max_value()).await {
              log::debug!("read {} bytes from stream", bytes.len());
              match out_sender.send((Instant::now(), bytes)).await {
                Err(_e) => {
                  log::error!("connection closed");
                  break;
                }
                _ => {}
              }
            }
          }
        },
        None => break,
      }
    }
    log::info!("exit recv loop");
    Err::<(), ()>(())
  };

  let _ = tokio::join!(a, b);
  log::info!("close stream: {}", addr);
}

pub mod server {
  use super::*;

  pub fn new_endpoint(bind_addr: SocketAddr) -> ResultType<(Endpoint, Incoming)> {
    let server_config =
      config(CERT.server_cert_file, CERT.server_key_file).expect("config quic server error");
    let (endpoint, incoming) = Endpoint::server(server_config, bind_addr)?;
    Ok((endpoint, incoming))
  }

  fn config(certs: &str, key_file: &str) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let roots = load_certs(certs);
    let certs = roots.clone();
    let mut client_auth_roots = rustls::RootCertStore::empty();
    for root in roots {
      client_auth_roots.add(&root).unwrap();
    }
    let client_auth = rustls::server::AllowAnyAuthenticatedClient::new(client_auth_roots);

    let privkey = load_private_key(key_file);
    let suites = rustls::ALL_CIPHER_SUITES.to_vec();
    let versions = rustls::ALL_VERSIONS.to_vec();

    let server_crypto = rustls::ServerConfig::builder()
      .with_cipher_suites(&suites)
      .with_safe_default_kx_groups()
      .with_protocol_versions(&versions)
      .expect("inconsistent cipher-suites/versions specified")
      .with_client_cert_verifier(client_auth)
      .with_single_cert_with_ocsp_and_sct(certs, privkey, vec![], vec![])
      .expect("bad certificates/private key");

    let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(server_crypto));
    let transport = Arc::get_mut(&mut server_config.transport).unwrap();
    transport.max_concurrent_bidi_streams(0_u8.into());
    transport.max_concurrent_uni_streams(500_u32.into());
    log::info!("quic server config: {:?}", &server_config);
    Ok(server_config)
  }
}

pub mod client {
  use super::*;

  pub(crate) fn config(ca_file: &str, certs_file: &str, key_file: &str) -> ClientConfig {
    let cert_file = File::open(&ca_file).expect(&format!("Cannot open CA file: {:?}", ca_file));
    let mut reader = BufReader::new(cert_file);

    let mut root_store = rustls::RootCertStore::empty();
    root_store.add_parsable_certificates(&rustls_pemfile::certs(&mut reader).unwrap());

    let suites = rustls::DEFAULT_CIPHER_SUITES.to_vec();
    let versions = rustls::DEFAULT_VERSIONS.to_vec();

    let certs = load_certs(certs_file);
    let key = load_private_key(key_file);

    let crypto = rustls::ClientConfig::builder()
      .with_cipher_suites(&suites)
      .with_safe_default_kx_groups()
      .with_protocol_versions(&versions)
      .expect("inconsistent cipher-suite/versions selected")
      .with_root_certificates(root_store)
      .with_single_cert(certs, key)
      .expect("invalid client auth certs/key");

    let mut client_config = ClientConfig::new(Arc::new(crypto));
    let transport = Arc::get_mut(&mut client_config.transport).unwrap();
    transport.max_concurrent_bidi_streams(0_u8.into());
    transport.max_concurrent_uni_streams(500_u32.into());
    log::info!("quic client config: {:?}", &client_config);
    client_config
  }
}

pub fn load_certs(filename: &str) -> Vec<rustls::Certificate> {
  let certfile =
    File::open(filename).expect(&format!("cannot open certificate file: {:?}", filename));
  let mut reader = BufReader::new(certfile);
  rustls_pemfile::certs(&mut reader)
    .unwrap()
    .iter()
    .map(|v| rustls::Certificate(v.clone()))
    .collect()
}

pub fn load_private_key(filename: &str) -> rustls::PrivateKey {
  let keyfile = File::open(filename).expect("cannot open private key file");
  let mut reader = BufReader::new(keyfile);

  loop {
    match rustls_pemfile::read_one(&mut reader).expect("cannot parse private key .pem file") {
      Some(rustls_pemfile::Item::RSAKey(key)) => return rustls::PrivateKey(key),
      Some(rustls_pemfile::Item::PKCS8Key(key)) => return rustls::PrivateKey(key),
      None => break,
      _ => {}
    }
  }

  panic!(
    "no keys found in {:?} (encrypted keys not supported)",
    filename
  );
}

#[cfg(test)]
mod tests {
  use std::{error::Error, net::SocketAddr};

  use futures_util::StreamExt;

  use super::*;

  #[tokio::test]
  async fn quic() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:5000".parse().unwrap();
    tokio::spawn(run_server(addr));
    run_client(addr).await;
    Ok(())
  }

  async fn run_server(addr: SocketAddr) {
    let (_endpoint, mut incoming) = server::new_endpoint(addr).unwrap();
    while let Some(conn) = incoming.next().await {
      tokio::spawn(handle_connection(conn));
    }
  }

  async fn handle_connection(conn: quinn::Connecting) {
    let mut conn = Connection::new_for_server(conn).await.unwrap();
    println!("[server] client address: {:?}", conn.remote_address());

    if let Some(recv_bytes) = conn.next().await {
      println!("[server] [2] recive: {:?}", recv_bytes);

      println!("[server] [3] send: hello client 1");
      conn
        .send_raw(b"hello client 1".to_vec())
        .await
        .unwrap_or_else(move |e| println!("failed: {reason}", reason = e.to_string()));
    }

    println!("[server] [5] send: hello client 2");
    conn.send_raw(b"hello client 2".to_vec()).await.unwrap();
    if let Some(resp_bytes) = conn.next().await {
      println!("[server] [8] receive: {:?}", resp_bytes);
    }
  }

  async fn run_client(server_addr: SocketAddr) {
    let local_addr = "127.0.0.1:8888".parse().unwrap();
    let mut conn = Connection::new_for_client(server_addr, local_addr, 1000)
      .await
      .unwrap();

    println!("[client] [1] send: hello server 1");
    let mut buf = BytesMut::with_capacity(64);
    buf.put(&b"hello server 1"[..]);
    conn.send_raw(b"hello server 1".to_vec()).await.unwrap();
    let resp_bytes = conn.next().await.unwrap();
    println!("[client] [4] receive: {:?}", resp_bytes);

    if let Some(recv_bytes) = conn.next().await {
      println!("[client] [6] recive: {:?}", recv_bytes);

      println!("[client] [7] send: hello server 2");
      conn
        .send_raw(b"hello server 2".to_vec())
        .await
        .unwrap_or_else(move |e| println!("failed: {reason}", reason = e.to_string()));
    }

    conn.shutdown().await.unwrap();
  }
}
