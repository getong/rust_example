use std::{
  fs::File,
  io::{self, BufReader},
  net::ToSocketAddrs,
  path::{Path, PathBuf},
  sync::Arc,
};

use argh::FromArgs;
use rustls_pemfile::{certs, rsa_private_keys};
// use rustls_pki_types::ServerName;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use tokio::{
  io::{copy, sink, split, AsyncWriteExt},
  net::TcpListener,
};
use tokio_rustls::TlsAcceptor;

/// Tokio Rustls server example
#[derive(FromArgs)]
struct Options {
  /// bind addr
  #[argh(positional)]
  addr: String,

  /// cert file
  #[argh(option, short = 'c')]
  cert: PathBuf,

  /// key file
  #[argh(option, short = 'k')]
  key: PathBuf,

  /// echo mode
  #[argh(switch, short = 'e')]
  echo_mode: bool,
}

fn load_certs(path: &Path) -> io::Result<Vec<CertificateDer<'static>>> {
  certs(&mut BufReader::new(File::open(path)?)).collect()
}

fn load_keys(path: &Path) -> io::Result<PrivateKeyDer<'static>> {
  rsa_private_keys(&mut BufReader::new(File::open(path)?))
    .next()
    .unwrap()
    .map(Into::into)
}

#[tokio::main]
async fn main() -> io::Result<()> {
  let options: Options = argh::from_env();

  let addr = options
    .addr
    .to_socket_addrs()?
    .next()
    .ok_or_else(|| io::Error::from(io::ErrorKind::AddrNotAvailable))?;
  let certs = load_certs(&options.cert)?;
  let key = load_keys(&options.key)?;
  let flag_echo = options.echo_mode;

  let config = rustls::ServerConfig::builder()
    .with_no_client_auth()
    .with_single_cert(certs, key)
    .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
  let acceptor = TlsAcceptor::from(Arc::new(config));

  let listener = TcpListener::bind(&addr).await?;

  loop {
    let (stream, peer_addr) = listener.accept().await?;
    let acceptor = acceptor.clone();

    let fut = async move {
      let mut stream = acceptor.accept(stream).await?;

      if flag_echo {
        let (mut reader, mut writer) = split(stream);
        let n = copy(&mut reader, &mut writer).await?;
        writer.flush().await?;
        println!("Echo: {} - {}", peer_addr, n);
      } else {
        let mut output = sink();
        stream
          .write_all(
            &b"HTTP/1.0 200 ok\r\n\
                           Connection: close\r\n\
                           Content-length: 12\r\n\
                           \r\n\
                           Hello world!"[..],
          )
          .await?;
        stream.shutdown().await?;
        copy(&mut stream, &mut output).await?;
        println!("Hello: {}", peer_addr);
      }

      Ok(()) as io::Result<()>
    };

    tokio::spawn(async move {
      if let Err(err) = fut.await {
        eprintln!("{:?}", err);
      }
    });
  }
}
