use anyhow::{anyhow, Result};
use sha3::{Digest, Keccak256};

pub const PEER_ID_LENGTH: usize = 20;

pub fn to_hex(original_str: &str) -> String {
  // with checksum encode
  let hex = hex::encode(original_str);

  let mut hasher = Keccak256::new();
  hasher.update(hex.as_bytes());
  let hash = hasher.finalize();
  let check_hash = hex::encode(&hash);

  let mut res = String::from("0x");
  for (index, byte) in hex[..PEER_ID_LENGTH * 2].chars().enumerate() {
    if check_hash.chars().nth(index).unwrap().to_digit(16).unwrap() > 7 {
      res += &byte.to_uppercase().to_string();
    } else {
      res += &byte.to_string();
    }
  }
  res
}

pub fn from_hex(s: &str) -> Result<String> {
  let raw = if s.starts_with("0x") { &s[2..] } else { s };
  match hex::decode(raw) {
    Ok(bytes) => Ok(String::from_utf8(bytes)?),
    Err(_) => Err(anyhow!("not decode")),
  }
}

fn main() {
  let hex = to_hex("hello world hello world hello world hello world hello world hello world");
  println!("hex is {:?}", hex);

  match from_hex(&hex) {
    Ok(decoded) => println!("Decoded string: {}", decoded),
    Err(e) => println!("Error: {}", e),
  }

  match from_hex("0x68656c6c6f") {
    Ok(decoded) => println!("Decoded string: {}", decoded),
    Err(e) => println!("Error: {}", e),
  }
}
