use std::sync::Arc;
use tokio::time::{sleep, Duration};

async fn add_to_vec<F>(shared_value: Arc<i32>, vec: &mut Vec<i32>, length: usize, mut f: F)
where
  F: FnMut(Arc<i32>, &mut Vec<i32>) + Send,
{
  loop {
    // Simulate some asynchronous work
    sleep(Duration::from_millis(100)).await;

    // Call the FnMut closure to add the shared value to the vector
    f(shared_value.clone(), vec);

    // Print the current state of the vector
    println!("Current vector: {:?}", vec);

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

  // Create a mutable vector
  let mut vec = Vec::new();

  // Specify the desired length
  let length = 5;

  // Define the FnMut closure
  let closure = |shared_value: Arc<i32>, vec: &mut Vec<i32>| {
    vec.push(*shared_value);
  };

  // Call the async function to add elements to the vector
  add_to_vec(shared_value, &mut vec, length, closure).await;

  // Print the final state of the vector
  println!("Final vector: {:?}", vec);
}
