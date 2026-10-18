use std::{
  io::{Read, Write},
  net::TcpStream,
  sync::Arc,
};

fn main() {
  let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
  let tls = rustls::ClientConfig::builder()
    .with_root_certificates(roots)
    .with_no_client_auth();
  let name = rustls::pki_types::ServerName::try_from("my.domain.io").expect("Invalid server name");
  let mut sess =
    rustls::ClientConnection::new(Arc::new(tls), name).expect("Could not create TLS connection");
  let mut conn = TcpStream::connect("my.domain.io:8000").unwrap();
  let mut stream = rustls::Stream::new(&mut sess, &mut conn);
  stream
    .write_all(concat!("GET /posts HTTP/1.1\r\n", "Connection: close\r\n", "\r\n").as_bytes())
    .expect("Could not write request");
  let mut plaintext = Vec::new();
  stream.read_to_end(&mut plaintext).expect("Could not read");
  println!(
    "{}",
    String::from_utf8(plaintext).expect("Could not print output")
  );
}
