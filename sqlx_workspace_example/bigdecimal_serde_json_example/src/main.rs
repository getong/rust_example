use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use serde_json;
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug)]
struct MyStruct {
  value: BigDecimal,
}

fn main() {
  // Create a BigDecimal value
  let input = "12345.6789";
  let big_decimal_value = BigDecimal::from_str(input).unwrap();

  // Create an instance of MyStruct
  let my_struct = MyStruct {
    value: big_decimal_value,
  };

  // Encode (serialize) MyStruct to a JSON string
  let json_string = serde_json::to_string(&my_struct).unwrap();
  println!("Serialized JSON: {}", json_string);

  // Decode (deserialize) the JSON string back to MyStruct
  let decoded_struct: MyStruct = serde_json::from_str(&json_string).unwrap();
  println!("Deserialized struct: {:?}", decoded_struct);
}
