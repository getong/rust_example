use std::sync::Arc;
//use std::thread;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> () {
    let listener = TcpListener::bind("127.0.0.1:6142").await.unwrap();
    let arc_listener = Arc::new(listener);

    for _i in 1..10 {
        let current_listener = Arc::clone(&arc_listener);
        tokio::spawn(async move {
            loop {
                let (mut socket, _) = current_listener.accept().await.unwrap();

                tokio::spawn(async move {
                    let mut buf = vec![0; 1024];

                    loop {
                        match socket.read(&mut buf).await {
                            // Return value of `Ok(0)` signifies that the remote has closed
                            Ok(0) => return,
                            Ok(n) => {
                                // Copy the data back to socket
                                if socket.write_all(&buf[..n]).await.is_err() {
                                    // Unexpected socket error. There isn't much we can
                                    // do here so just stop processing.
                                    return;
                                }
                            }
                            Err(_) => {
                                // Unexpected socket error. There isn't much we can do
                                // here so just stop processing.
                                return;
                            }
                        }
                    }
                });
            }
        })
        .await
        .unwrap();
    }
}
