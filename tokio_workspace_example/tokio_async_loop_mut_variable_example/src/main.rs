use std::{future::Future, sync::Arc};
use tokio::{
  sync::Mutex,
  time::{sleep, Duration},
};

async fn add_to_vec<F, Fut>(
  shared_value: Arc<i32>,
  vec: Arc<Mutex<Vec<i32>>>,
  length: usize,
  mut f: F,
) where
  F: FnMut(Arc<i32>, Arc<Mutex<Vec<i32>>>) -> Fut + Send,
  Fut: Future<Output = ()> + Send + 'static,
{
  loop {
    // Simulate some asynchronous work
    sleep(Duration::from_millis(100)).await;

    // Call the async FnMut closure to add the shared value to the vector
    f(shared_value.clone(), vec.clone()).await;

    // Print the current state of the vector
    let vec = vec.lock().await;
    println!("Current vector: {:?}", *vec);

    // Break the loop if the vector's length reaches the specified length
    if vec.len() >= length {
      break;
    }
  }
}

#[tokio::main]
async fn main() {
  // Create an Arc<i32> to share the value
  let shared_value = Arc::new(42);

  // Create a mutable vector wrapped in a Mutex and Arc
  let vec = Arc::new(Mutex::new(Vec::new()));

  // Specify the desired length
  let length = 5;

  // Define the async FnMut closure
  let closure = |shared_value: Arc<i32>, vec: Arc<Mutex<Vec<i32>>>| {
    Box::pin(async move {
      let mut vec = vec.lock().await;
      vec.push(*shared_value);
    })
  };

  // Call the async function to add elements to the vector
  add_to_vec(shared_value, vec.clone(), length, closure).await;

  // Print the final state of the vector
  let vec = vec.lock().await;
  println!("Final vector: {:?}", *vec);
}
