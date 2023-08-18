use std::io::Result;
use tokio::net::UdpSocket;
use tokio::time::Duration;

// copy from https://www.reddit.com/r/rust/comments/15tiukb/tokios_udpsockettry_send_toggles_between_err_and/
// nc -l -u localhost 10000
#[tokio::main]
async fn main() -> Result<()> {
    let udp_socket = UdpSocket::bind("0.0.0.0:10001").await?;
    udp_socket.connect("0.0.0.0:10000").await?;

    let mut buf = [0; 1024];
    let mut heartbeat = tokio::time::interval(Duration::from_secs(1));

    let handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = udp_socket.readable() => {
                    match udp_socket.try_recv(&mut buf) {
                        Ok(n) => println!("got {:x?}", &buf[..n]),
                        Err(e) => eprintln!("failed to read {e:?}"),
                    }
                }
                _ = heartbeat.tick() => {
                    println!("sending heartbeat {:?}", udp_socket.try_send(&[32]));
                }
            }
        }
    });
    tokio::try_join!(handle).unwrap();
    Ok(())
}
