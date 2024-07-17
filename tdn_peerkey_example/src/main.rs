use tdn::prelude::PeerKey;

fn main() {
  let mut rand = rand::thread_rng();
  let key = PeerKey::generate(&mut rand);
  let hex = key.to_db_bytes();

  println!("hex is {:?}", hex);
  let hex_len = hex.len();
  println!("hex_len is {:?}", hex_len);
  let hex_str = hex::encode(hex);
  // hex_str is 01aa3954e9789545acb6b749b0c2ff42b1c972caee22194d2fdd214b70e9356a
  println!("hex_str is {}", hex_str);
}
