use anyhow::{anyhow, Result};

pub fn to_hex(original_str: &str) -> String {
  // with checksum encode
  let hex = hex::encode(original_str);

  let mut res = String::from("0x");
  res.push_str(hex.to_lowercase().as_str());
  res
}

pub fn from_hex(s: &str) -> Result<String> {
  let raw = if s.starts_with("0x") { &s[2 ..] } else { s };
  match hex::decode(raw) {
    Ok(bytes) => Ok(String::from_utf8(bytes)?),
    Err(_) => Err(anyhow!("not decode")),
  }
}

fn main() {
  let hex = to_hex("hello world");
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
