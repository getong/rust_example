use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("0.0.0.0:8080").await.unwrap();
    let sem = Arc::new(Semaphore::new(2));

    loop {
        let (mut tcp_stream, _) = listener.accept().await.unwrap();
        let sem_clone = Arc::clone(&sem);
        tokio::spawn(async move {
            let aq = sem_clone.try_acquire();
            if let Ok(_guard) = aq {
                let mut buf: [u8; 1024] = [0; 1024];
                loop {
                    let n = tcp_stream.read(&mut buf).await.unwrap();
                    if n == 0 {
                        return;
                    }
                    print!("{}", String::from_utf8_lossy(&buf[0..n]));
                }
            } else {
                println!("Rejecting client: too many open sockets");
            }
        });
    }
}
