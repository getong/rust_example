use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    let x = Arc::new(RwLock::new(tx));
    let x_clone = x.clone();

    tokio::spawn(async move {
        _ = x_clone.read().await.send(4u8);
    });

    let x_clone2 = x.clone();

    tokio::spawn(async move {
        _ = x_clone2.read().await.send(5u8);
    });

    _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    while let Some(i) = rx.recv().await {
        println!("recv i:{}", i);
    }
}
