use std::error::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind("127.0.0.1:8080").await?;

    println!("Server started, listening on 127.0.0.1:8080");

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(async move {
            let mut buffer = vec![0u8; 1024];

            loop {
                match socket.read(&mut buffer).await {
                    Ok(0) => {
                        println!("client disconnect");
                        break;
                    }
                    Ok(n) => {
                        eprintln!("Write n: {}, total: {:?}", n, &buffer[..n]);
                        if let Err(e) = socket.write_all(&buffer[..n]).await {
                            eprintln!("Write error: {}", e);
                            break;
                        }
                        // buffer = vec![0u8; 1024];
                    }
                    Err(e) => {
                        eprintln!("Read error: {}", e);
                        break;
                    }
                }
            }
        });
    }
}
