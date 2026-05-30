use std::time::Duration;

use rand::RngExt;
use tokio::time::sleep;

#[tokio::main]
async fn main() {
  loop {
    tokio::select! {
        _ = sleep(Duration::from_secs(1)) => {
            println!("1 second has passed");
        }

        _ = generate_random_number() => {
            println!("Random number generated");
        }
    }
  }
}

async fn generate_random_number() {
  let mut rng = rand::rng();
  let number: u32 = rng.random();
  println!("Generated random number: {}", number);
}
