use tokio::sync::broadcast;

#[tokio::main]
async fn main() {
  let (tx, mut rx1) = broadcast::channel(16);
  let mut rx2 = tx.subscribe();

  tokio::spawn(async move {
    assert_eq!(rx1.recv().await.unwrap(), 10);
    assert_eq!(rx1.recv().await.unwrap(), 20);
  });

  tokio::spawn(async move {
    assert_eq!(rx2.recv().await.unwrap(), 10);
    assert_eq!(rx2.recv().await.unwrap(), 20);
  });

  tx.send(10).unwrap();
  tx.send(20).unwrap();
}
