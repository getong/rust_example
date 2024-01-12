// #![feature(once_cell)]
#![feature(lazy_cell)]

use std::collections::HashMap;

use std::sync::LazyLock;

static HASHMAP: LazyLock<HashMap<i32, String>> = LazyLock::new(|| {
  println!("initializing");
  let mut m = HashMap::new();
  m.insert(13, "Spica".to_string());
  m.insert(74, "Hoyten".to_string());
  m
});

fn main() {
  println!("ready");
  std::thread::spawn(|| {
    println!("{:?}", HASHMAP.get(&13));
  })
  .join()
  .unwrap();
  println!("{:?}", HASHMAP.get(&74));

  // Prints:
  //   ready
  //   initializing
  //   Some("Spica")
  //   Some("Hoyten")
}
