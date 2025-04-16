use ethers_core::types::H256;

pub fn h256_hex(a: &H256) -> String {
  // Access the underlying bytes of H256 directly using .0
  hex::encode(a.0)
}

pub fn hex_h256(a: &str) -> H256 {
  let bytes = hex::decode(a).unwrap_or_else(|_| vec![0u8; 32]);
  let mut padded_bytes = [0u8; 32];
  padded_bytes[.. bytes.len()].copy_from_slice(&bytes);
  H256::from(padded_bytes)
}

fn main() {
  let h256 = hex_h256("4bd6dacf2d5a5c93f410964569cddab068bb038ce976e53818cc032181cb8373");
  println!("h256 is {:?}", h256);

  let h256 = h256_hex(&H256::from_low_u64_be(11556959009357346568));
  println!("h256 is {:#?}", h256);

  match hex::decode("4bd6dacf2d5a5c93f410964569cddab068bb038ce976e53818cc032181cb8373") {
    Ok(decoded) => println!("Decoded bytes: {:?}", decoded),
    Err(e) => println!("Failed to decode: {}", e),
  }

  let new_h256 = h256_hex(&H256::from(hex_h256(
    "4bd6dacf2d5a5c93f410964569cddab068bb038ce976e53818cc032181cb8373",
  )));
  println!("new h256 is {:#?}", new_h256);
}
