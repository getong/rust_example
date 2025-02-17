use std::sync::Arc;

use tokio::{
  sync::Mutex,
  time::{Duration, timeout},
};

static GLOBAL_MUTEX: once_cell::sync::Lazy<Arc<Mutex<i32>>> =
  once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(0)));

#[tokio::main]
async fn main() {
  let mutex = GLOBAL_MUTEX.clone();

  let handle = tokio::spawn(async move {
    let _lock = mutex.lock().await; // Lock is held indefinitely
    tokio::time::sleep(Duration::from_secs(10)).await; // Simulate long work
  });

  tokio::time::sleep(Duration::from_secs(1)).await; // Ensure the first task acquires the lock

  let mutex = GLOBAL_MUTEX.clone();
  match timeout(Duration::from_secs(5), mutex.lock()).await {
    Ok(_lock) => println!("Successfully acquired the lock"),
    Err(_) => println!("Failed to acquire the lock within timeout"),
  }

  handle.await.unwrap();
}
