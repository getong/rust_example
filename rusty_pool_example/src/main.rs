use rusty_pool::Builder;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;

// use std::time::Duration;
const BATCH_SIZE: usize = 10;

async fn some_async_fn(x: i32, y: i32) -> i32 {
  x + y
}

async fn other_async_fn(x: i32, y: i32) -> i32 {
  x - y
}

#[tokio::main]
async fn main() {
  let pool = Builder::new()
    .core_size(num_cpus::get())
    .max_size(BATCH_SIZE)
    .build();

  let count = Arc::new(AtomicI32::new(0));

  let clone = count.clone();

  pool.spawn(async move {
    let a = some_async_fn(3, 6).await; // 9
    let b = other_async_fn(a, 4).await; // 5
    let c = some_async_fn(b, 7).await; // 12
    clone.fetch_add(c, Ordering::SeqCst);
  });
  pool.join();
  assert_eq!(count.load(Ordering::SeqCst), 12);

  // simply complete future by blocking a worker until the future has been completed
  let handle = pool.complete(async {
    let a = some_async_fn(4, 6).await; // 10
    let b = some_async_fn(a, 3).await; // 13
    let c = other_async_fn(b, a).await; // 3
    some_async_fn(c, 5).await // 8
  });
  assert_eq!(handle.await_complete(), 8);

  for i in 0 ..= 100 {
    pool.spawn(async move {
      println!("i is {}", i);
    });
  }
  // pool.join will wait for all the futures to complete
  // if no pool.join, some futures will not run completely
  pool.join();
}
