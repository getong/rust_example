use std::sync::Arc;

use rand::{rngs::OsRng, Rng};
use tokio::{
  sync::{oneshot, Mutex},
  time::{timeout, Duration},
};

#[tokio::main]
async fn main() {
  // Create a oneshot channel
  let (tx, rx) = oneshot::channel();

  // Create a random sleep duration outside the async block
  // let mut rng = rand::thread_rng();
  // let sleep_duration = Duration::from_secs(rng.gen_range(1..3));

  // Create an Arc to safely share the random number generator
  let rng = Arc::new(Mutex::new(OsRng::default()));

  let rng_clone = Arc::clone(&rng);
  // Spawn a Tokio task to send a value through the channel after a delay
  tokio::spawn(async move {
    let message = "Hello from the other side!";

    // let mut rng = rand::thread_rng();
    let mut rng = rng_clone.lock().await;
    let random_number = rng.gen_range(1 ..= 3);
    let sleep_duration = Duration::from_secs(random_number);

    // Generate random sleep time
    tokio::time::sleep(sleep_duration).await;
    // tokio::time::sleep(Duration::from_secs(3)).await; // Simulate delay
    let _ = tx.send(message);
  });

  // Use timeout to receive the value with a timeout
  let timeout_duration = Duration::from_secs(2);
  match timeout(timeout_duration, rx).await {
    Ok(result) => match result {
      Ok(value) => {
        println!("Received: {}", value);
      }
      Err(e1) => {
        println!(
          "Timed out: Value not received within the timeout, e:{:?}",
          e1
        );
      }
    },
    Err(e) => {
      println!("Error receiving value, the e: {:?}", e);
    }
  }
}
