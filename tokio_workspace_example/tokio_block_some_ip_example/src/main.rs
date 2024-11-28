use std::net::{IpAddr, Ipv4Addr};

use tokio::net::{TcpListener, TcpStream};
// use tokio_stream::StreamExt;

async fn handle_connection(stream: TcpStream) {
  // Handle the incoming connection here
  println!("New connection from {:?}", stream.peer_addr().unwrap());
  // ... do something with the connection ...
}

async fn filter_ip_block(addr: IpAddr) -> bool {
  // Define your IP block range here
  let blocked_ip_start = IpAddr::V4(Ipv4Addr::new(192, 168, 0, 0));
  let blocked_ip_end = IpAddr::V4(Ipv4Addr::new(192, 168, 255, 255));

  addr >= blocked_ip_start && addr <= blocked_ip_end
}

#[tokio::main]
async fn main() {
  let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();

  loop {
    let (stream, addr) = listener.accept().await.unwrap();

    // Filter connections from blocked IP addresses
    if filter_ip_block(addr.ip()).await {
      println!("Blocked connection from {:?}", addr);
      continue;
    }

    tokio::spawn(async move {
      handle_connection(stream).await;
    });
  }
}
