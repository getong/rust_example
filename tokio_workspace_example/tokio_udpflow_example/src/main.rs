use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;
use udpflow::{UdpListener, UdpStreamLocal, UdpStreamRemote};

#[tokio::main]
async fn main() {
  let socket = UdpSocket::bind("127.0.0.1:5000").await.unwrap();
  let listener = UdpListener::new(socket);
  let mut buf = vec![0u8; 0x2000];
  // listener must be continuously polled to recv packets or accept new streams
  while let Ok((stream, _addr)) = listener.accept(&mut buf).await {
    tokio::spawn(handle(stream));
  }
}

async fn handle(mut stream1: UdpStreamLocal) {
  let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
  let mut stream2 = UdpStreamRemote::new(socket, "127.0.0.1:10000".parse().unwrap());
  loop {
    let mut buf = vec![0u8; 256];
    let _ = stream1.read(&mut buf).await;
    let _ = stream2.write(&buf).await;
    let _ = stream2.read(&mut buf).await;
    let _ = stream1.write(&buf).await;
  }
}
