use std::sync::Arc;

use tokio::sync::{OnceCell, RwLock};

async fn some_computation() -> u32 {
  1 + 1
}

static ONCE_LIST: OnceCell<Arc<RwLock<Vec<u32>>>> = OnceCell::const_new();

async fn get_global_vector() -> Arc<RwLock<Vec<u32>>> {
  ONCE_LIST
    .get_or_init(|| async { Arc::new(RwLock::new(vec![1, 2, 3])) })
    .await
    .clone()
}

async fn set_global_vector(value: Vec<u32>) {
  let lock = Arc::new(RwLock::new(value));
  ONCE_LIST.set(lock).unwrap_or_else(|_| {
    println!("ONCE_LIST has already been set");
  });
}

static ONCE: OnceCell<u32> = OnceCell::const_new();

async fn get_global_integer() -> &'static u32 {
  ONCE.get_or_init(|| async { 1 + 1 }).await
}

#[tokio::main]
async fn main() {
  let result = ONCE.get_or_init(some_computation).await;
  assert_eq!(*result, 2);

  let result = get_global_integer().await;
  assert_eq!(*result, 2);

  // Set the value of ONCE_LIST
  set_global_vector(vec![42, 43, 44]).await;

  // Retrieve the value of ONCE_LIST
  let lock = get_global_vector().await;
  {
    let read_guard = lock.read().await;
    assert_eq!(*read_guard, vec![42, 43, 44]);

    for i in &*read_guard {
      println!("i is {}", i);
    }
  } // `read_guard` is dropped here

  // Mutate the value of ONCE_LIST
  let mut write_guard = lock.write().await;
  write_guard.push(100);
  drop(write_guard);

  // Retrieve the mutated value of ONCE_LIST
  let read_guard = lock.read().await;
  assert_eq!(*read_guard, vec![42, 43, 44, 100]);
  drop(read_guard);

  let new_data = Arc::new(RwLock::new(vec![4, 5, 6]));
  _ = ONCE_LIST.set(new_data);

  let lock = get_global_vector().await;
  let read_guard = lock.read().await;
  assert_eq!(*read_guard, vec![43, 43, 44, 100]);
}
