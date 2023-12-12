use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
  let start = SystemTime::now();
  let since_the_epoch = start
    .duration_since(UNIX_EPOCH)
    .expect("Time went backwards");
  println!("{:?}", since_the_epoch);
}
