use std::sync::Arc;

use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
  let data1 = Arc::new(Mutex::new(0));
  let data2 = data1.clone();
  loop {
    let data3 = data2.clone();
    tokio::spawn(async move {
      let mut lock = data3.lock().await;
      *lock += 1;
    });
  }
}
