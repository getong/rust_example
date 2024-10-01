use alloy_primitives::U256;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Define a struct for the nonce manager.
#[derive(Clone)]
struct NonceManager {
  nonces: Arc<Mutex<HashMap<String, U256>>>, // Using HashMap to store nonce for each address
}

impl NonceManager {
  // Create a new NonceManager.
  fn new() -> Self {
    NonceManager {
      nonces: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  // Get the next nonce for a given address.
  fn get_next_nonce(&self, address: &str) -> U256 {
    let mut nonces = self.nonces.lock().unwrap();
    let nonce = nonces.entry(address.to_string()).or_insert(U256::from(0));
    let next_nonce = *nonce;
    *nonce += U256::from(1); // Increment nonce
    next_nonce
  }

  // Set a specific nonce for an address.
  fn set_nonce(&self, address: &str, nonce: U256) {
    let mut nonces = self.nonces.lock().unwrap();
    nonces.insert(address.to_string(), nonce);
  }
}

fn main() {
  let manager = NonceManager::new();

  // Example address
  let address = "0xabc123";

  // Get the next nonce for this address
  let nonce1 = manager.get_next_nonce(address);
  println!("Nonce 1: {}", nonce1);

  let nonce2 = manager.get_next_nonce(address);
  println!("Nonce 2: {}", nonce2);

  // Set a custom nonce
  manager.set_nonce(address, U256::from(42));
  let custom_nonce = manager.get_next_nonce(address);
  println!("Custom Nonce: {}", custom_nonce);
}
