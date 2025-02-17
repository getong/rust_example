use std::collections::HashMap;

use ethers::{
  abi::RawLog,
  contract::EthEvent,
  types::{Address, Bytes, H256, U256},
};
use hex::FromHex;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};

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

#[allow(non_snake_case)]
#[derive(Clone, Debug, EthEvent)]
struct ChannelOpen {
  #[ethevent(indexed)]
  channelId: U256,
  indexer: Address,
  consumer: Address,
  total: U256,
  price: U256,
  expiredAt: U256,
  deploymentId: H256,
  callback: Bytes,
}

fn decode_ether_byte() {
  // Hexadecimal string to byte array conversion
  let hex_string = "000000000000000000000000a10af672bcdd1dd61b6a63a18295e55e5f3ea842000000000000000000000000ed5fc5a4ad3e952291fe02b223b137c5d212266f0000000000000000000000000000000000000000000000001bc16d674ec8000000000000000000000000000000000000000000000000000000038d7ea4c680000000000000000000000000000000000000000000000000000000000067245590c26f8e11da9bb4e0bcc1f25044859c5a35f05a4405ce24446d5d3dc993d7899300000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000060000000000000000000000000bf3a286a477967ebd850cee2dbdbfa6e535a9e6400000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000";

  // Convert hex string to byte vector
  let byte_data = Bytes::from_static(hex_string);

  // Example topics (in reality, these would come from the log you're decoding)
  let topics: Vec<H256> = vec![
    H256::from_slice(&[0u8; 32]), // Placeholder for the actual topic
  ];

  // Create the RawLog from topics and byte data
  let raw_log = RawLog {
    topics,
    data: byte_data, // Vec<u8> is required here
  };

  // Decode bytes into ChannelOpen event
  let event = ChannelOpen::decode_log(&raw_log).expect("Failed to decode event");

  println!("Decoded event: {:?}", event);
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

  // Uncomment this line, it will throw an error because "1.1" is not a valid decimal for U256
  // let total_value = U256::from_dec_str("1.1").expect("Failed to parse decimal");
  // println!("total_value : {:?}", total_value);

  let a = U256::from(5636815);
  let b = U256::from(2_000_000);
  let c = U256::from(1_000_000);
  let d = a * b / c;
  println!("d is {}", d);

  // Test the byte decoding
  decode_ether_byte();
}
