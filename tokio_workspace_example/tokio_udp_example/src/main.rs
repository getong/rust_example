use std::{io, net::SocketAddr};
use tokio::{
    net::UdpSocket,
    time::{self, Duration},
};

async fn listen_udp(addr: SocketAddr) -> io::Result<()> {
    let udp = UdpSocket::bind(addr).await?;
    let mut buf = [0; 4096];

    match time::timeout(Duration::from_secs(5), udp.recv_from(&mut buf)).await? {
        Ok((count, src)) => {
            udp.send_to(&buf[..count], &src).await?;
            println!("Message recv: {:?}", &buf[..count]);
        }
        Err(e) => {
            eprintln!("timed out: {:?}", e);
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    listen_udp("127.0.0.1:9953".parse()?).await?;
    Ok(())
}
