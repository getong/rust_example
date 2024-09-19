use sentry::types::protocol::v7::{Exception, Values};
use sha2::{Digest, Sha256};

fn hash_values(values: &Values<Exception>) -> String {
  // Serialize the `Values` instance to a JSON string
  let serialized = serde_json::to_string(values).unwrap();

  // Create a SHA-256 hasher instance
  let mut hasher = Sha256::new();

  // Write the serialized data into the hasher
  hasher.update(serialized);

  // Convert the hash result to a hexadecimal string
  let result = hasher.finalize();
  format!("{:x}", result)
}

fn main() {
  // Example of creating a `Values` instance
  // Replace this with the actual initialization of `Values` if needed
  let values = Values::new(); // Assuming `Values` has a `new` method

  // Compute the hash of the `Values` instance
  let hash = hash_values(&values);

  // Print the resulting hash
  println!("Hash of the `Values` instance: {}", hash);
}
