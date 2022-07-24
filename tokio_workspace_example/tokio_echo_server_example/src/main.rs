use std::error::Error;

use tokio::io::copy;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = "127.0.0.1:6143";
    let listener = TcpListener::bind(addr).await?;
    println!("Listen on {}", addr);
    loop {
        let (mut sock, _) = listener.accept().await?;
        tokio::spawn(async move {
            let (mut reader, mut writer) = sock.split();
            copy(&mut reader, &mut writer).await.unwrap();
        });
    }
}
