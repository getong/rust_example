use alloy::primitives::U256;

fn u256_hex(a: &U256) -> String {
  // let mut bytes = [0u8; 32];
  let bytes: [u8; 32] = a.to_be_bytes();
  hex::encode(bytes)
}

fn main() {
  let number = U256::from_str_radix(
    "19437480689627827356870340962242074444005882000807170425114224420327980784499",
    10,
  )
  .unwrap();
  let number = u256_hex(&number);
  println!("number is {}", number);
}
