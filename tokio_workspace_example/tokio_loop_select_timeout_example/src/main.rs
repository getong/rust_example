use std::net::SocketAddr;

use tokio::{
  net::UdpSocket,
  time::{self, Duration},
};

async fn listen_udp(addr: SocketAddr) {
  let udp = UdpSocket::bind(addr).await.unwrap();
  let mut buf = [0; 4096];

  loop {
    tokio::select! {
        r = time::timeout(Duration::from_secs(5), udp.recv_from(&mut buf)) => {
            match r {
                Ok(Ok((count, src))) => {
                    let _ = udp.send_to(&buf[..count], &src).await;
                    println!("Message recv: {:?}", &buf[..count]);
                },
                Err(e) => {
                    eprintln!("Err timed out: {:?}", e);
                },
                Ok(Err(e)) => {
                    eprintln!("ok but error occur, timed out: {:?}", e);
                },
            }
        },
        else => {
            println!("other info");
        }
    }
  }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  listen_udp("127.0.0.1:9953".parse()?).await;
  Ok(())
}
