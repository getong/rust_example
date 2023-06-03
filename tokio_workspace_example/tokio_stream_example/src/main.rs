use tokio::sync::watch;
use tokio_stream::wrappers::WatchStream;
use tokio_stream::StreamExt;

#[tokio::main]
async fn main() {
    let mut stream = tokio_stream::iter(&[1, 2, 3]);

    while let Some(v) = stream.next().await {
        println!("GOT = {:?}", v);
    }

    let (tx, rx) = watch::channel("hello");
    let mut rx = WatchStream::new(rx);

    assert_eq!(rx.next().await, Some("hello"));

    tx.send("goodbye").unwrap();
    assert_eq!(rx.next().await, Some("goodbye"));

    let (tx, rx) = watch::channel("hello");
    let mut rx = WatchStream::new(rx);

    tx.send("goodbye").unwrap();
    assert_eq!(rx.next().await, Some("goodbye"));
}
