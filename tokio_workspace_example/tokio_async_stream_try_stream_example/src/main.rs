use async_stream::try_stream;
use futures_core::stream::Stream;
use std::io;
use std::net::SocketAddr;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio::time::Duration;
use tokio_stream::StreamExt;

const READ_TIMEOUT_SECONDS: u64 = 5;

async fn bind_and_accept(addr: SocketAddr) -> impl Stream<Item = io::Result<TcpStream>> {
    try_stream! {
        let  listener = TcpListener::bind(addr).await?;

        loop {
            let (stream, addr) = listener.accept().await?;
            println!("received on {:?}", addr);
            yield stream;
        }
    }
}

#[tokio::main]
async fn main() {
    let addr = "127.0.0.1:3000".parse().unwrap();
    let stream = bind_and_accept(addr).await;
    tokio::pin!(stream);
    while let Some(Ok(client_stream)) = stream.next().await {
        println!("data: {:?}", client_stream);
        tokio::spawn(async move {
            if let Err(err) = handle_client(client_stream).await {
                eprintln!("Error: {}", err);
            }
        });
    }
}

async fn handle_client(mut client_stream: TcpStream) -> Result<(), io::Error> {
    let mut buffer: Vec<u8> = vec![0u8; 1024];
    // let mut buffer =[0u8;1024];

    loop {
        let read_future = client_stream.read(&mut buffer);
        tokio::select! {
            result = timeout(Duration::from_secs(READ_TIMEOUT_SECONDS), read_future) => {
                if let Ok(Ok(nbytes)) =result {
                    if nbytes == 0 {
                        // End of stream, client disconnected
                        return Ok(());
                    }

                    // Process the received data
                    let data = String::from_utf8_lossy(&buffer[..nbytes]);
                    println!("Received data from client: {}", data);

                    // Echo the data back to the client
                    if let Err(err) = client_stream.write_all(&buffer[..nbytes]).await {
                        eprintln!("Write error: {}", err);
                        return Err(err);
                    }
                }
            },
        }
    }
}
