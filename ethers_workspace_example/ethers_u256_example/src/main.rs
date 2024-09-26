use ethers::types::U256;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
struct Payload {
  total: String,
}

fn u256_to_i32(value: U256) -> Result<i32, String> {
  if value > U256::from(i32::MAX) {
    return Err("Value too large to fit in i32".to_string());
  }
  Ok(value.low_u32() as i32)
}

fn main() {
  // Create a payload and insert the string value
  let mut payload = HashMap::new();
  payload.insert("total", "123".to_string());

  // Serialize the payload to a JSON string
  let serialized = to_string(&payload).expect("Failed to serialize payload");
  println!("Serialized JSON: {}", serialized);

  // Deserialize the JSON string back to a HashMap
  let deserialized: HashMap<String, String> =
    from_str(&serialized).expect("Failed to deserialize JSON");
  println!("Deserialized payload: {:?}", deserialized);

  // Extract the string and convert to U256
  let total_str = deserialized.get("total").expect("Key 'total' not found");
  let total_value = U256::from_dec_str(total_str).expect("Failed to parse decimal string to U256");

  println!("total_value: {}", total_value);

  // Example U256 value
  let u256_value = U256::from(12345);

  // Convert U256 to i32
  let i32_value = u256_to_i32(u256_value).expect("Value out of range for i32");

    println!("Converted value: {}", i32_value);

    let total_value = U256::from_dec_str("1.1");
    println!("total_value : {:?}", total_value);
}
