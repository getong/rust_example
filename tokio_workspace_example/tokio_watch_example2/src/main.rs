use tokio::sync::watch;

#[tokio::main]
async fn main() {
  let (tx, mut rx) = watch::channel("hello");

  tokio::spawn(async move {
    tx.send("goodbye").unwrap();
  });

  assert!(rx.changed().await.is_ok());
  assert_eq!(*rx.borrow(), "goodbye");

  // The `tx` handle has been dropped
  assert!(rx.changed().await.is_err());
}
