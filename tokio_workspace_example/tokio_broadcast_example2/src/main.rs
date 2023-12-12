use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
  let (tx, mut rx) = broadcast::channel(16);

  // Will not be seen
  tx.send(10).unwrap();

  let value = rx.recv().await.unwrap();
  assert_eq!(10, value);

  // a new receiver
  let mut rx2 = tx.subscribe();

  tx.send(20).unwrap();

  let value = rx2.recv().await.unwrap();
  assert_eq!(20, value);
}
