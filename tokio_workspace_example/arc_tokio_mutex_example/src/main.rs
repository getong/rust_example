use tokio::sync::Mutex;
use std::sync::Arc;

#[tokio::main]
async fn main() {
  let data1 = Arc::new(Mutex::new(0));
  let data2 = Arc::clone(&data1);

  tokio::spawn(async move {
    let mut lock = data2.lock().await;
    *lock += 1;
  });

  let mut lock = data1.lock().await;
  *lock += 1;
}
