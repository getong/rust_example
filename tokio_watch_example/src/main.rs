use tokio::sync::watch;

#[tokio::main]
async fn main() {
    // println!("Hello, world!");
    let (tx, mut rx) = watch::channel("hello");

    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            println!("received = {:?}", *rx.borrow());
        }
    });

    let _ = tx.send("world");
}
