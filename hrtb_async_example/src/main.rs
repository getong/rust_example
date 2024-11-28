// use std::future::Future;
use futures::{future::BoxFuture, lock::Mutex};

pub async fn with_mutex<I, O, F>(mutex: &Mutex<I>, f: F) -> O
where
  F: for<'a> FnOnce(&'a mut I) -> BoxFuture<'a, O>,
{
  let mut guard = mutex.lock().await;
  f(&mut guard).await
}

pub async fn run() {
  let mutex = Mutex::new(5);
  println!("mutex: {:?}", mutex.lock().await);
  let _fut = with_mutex(&mutex, |value| {
    Box::pin(async {
      *value += 1;
    })
  })
  .await;
  println!("mutex: {:?}", mutex.lock().await);
}

#[tokio::main]
async fn main() {
  run().await;
}
