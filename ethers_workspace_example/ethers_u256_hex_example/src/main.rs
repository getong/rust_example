use ethers::types::U256;

fn u256_hex(a: &U256) -> String {
  let mut bytes = [0u8; 32];
  a.to_big_endian(&mut bytes);
  hex::encode(bytes)
}

// fn hex_u256(a: &str) -> U256 {
//     let bytes = hex::decode(a).unwrap_or(vec![0u8; 32]);
//     U256::from_big_endian(&bytes)
// }

fn main() {
  let number = U256::from_dec_str(
    "19437480689627827356870340962242074444005882000807170425114224420327980784499",
  )
  .unwrap();
  let number = u256_hex(&number);
  println!("number is {}", number);
}
