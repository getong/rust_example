use std::{collections::HashMap, sync::Mutex};

use once_cell::{sync, unsync};

static GLOBAL_DATA: sync::Lazy<Mutex<HashMap<i32, String>>> = sync::Lazy::new(|| {
  let mut m = HashMap::new();
  m.insert(13, "Spica".to_string());
  m.insert(74, "Hoyten".to_string());
  Mutex::new(m)
});

static HASHMAP: sync::Lazy<HashMap<i32, String>> = sync::Lazy::new(|| {
  println!("initializing");
  let mut m = HashMap::new();
  m.insert(13, "Spica".to_string());
  m.insert(74, "Hoyten".to_string());
  m
});

fn main() {
  println!("{:?}", GLOBAL_DATA.lock().unwrap());

  println!("ready");
  std::thread::spawn(|| {
    assert_eq!(HASHMAP.get(&13), Some(&"Spica".to_owned()));
  })
  .join()
  .unwrap();
  assert_eq!(HASHMAP.get(&74), Some(&"Hoyten".to_owned()));

  let lazy: unsync::Lazy<i32> = unsync::Lazy::new(|| {
    println!("initializing");
    92
  });

  println!("unsync ready");
  assert_eq!(*lazy, 92);
  assert_eq!(*lazy, 92);
}
