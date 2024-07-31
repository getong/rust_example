use ethers::types::U256;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
struct Payload {
  total: U256,
}

fn main() {
  // Create a U256 from a decimal string
  let total_value = U256::from_dec_str("123456789012345678901234567890")
    .expect("Failed to parse decimal string to U256");

  // Create a payload and insert the U256 value
  let mut payload = HashMap::new();
  payload.insert("total", total_value);

  // Serialize the payload to a JSON string
  let serialized = to_string(&payload).expect("Failed to serialize payload");
  println!("Serialized JSON: {}", serialized);

  // Deserialize the JSON string back to a HashMap
  let deserialized: HashMap<String, U256> =
    from_str(&serialized).expect("Failed to deserialize JSON");
  println!("Deserialized payload: {:?}", deserialized);
}

// fn main() {
//   let u256 = U256::MAX;
//   let h160 = H160::repeat_byte(0x0F);

//   println!("{u256:#032X}");
//   println!("{u256:#032x}");

//   println!("{h160:#020X}");
//   println!("{h160:#020x}");

//   // Example decimal string
//   let decimal_string = "123456789012345678901234567890";

//   // Create a U256 from the decimal string
//   let value = U256::from_dec_str(decimal_string)
//     .expect("Failed to parse decimal string to U256");

//   // Print the U256 value
//   println!("The U256 value is: {}", value);
// }
