use tokio::time;

#[tokio::main]
async fn main() {
  let handle1 = tokio::spawn(async {
    // do some stuff here
  });

  let handle2 = tokio::spawn(async {
    // do some other stuff here
    time::sleep(time::Duration::from_secs(10)).await;
  });
  // Wait for the task to finish
  handle2.abort();
  time::sleep(time::Duration::from_secs(1)).await;
  assert!(handle1.is_finished());
  assert!(handle2.is_finished());
}
