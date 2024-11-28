// fn main() {
//     println!("Hello, world!");
// }

use std::collections::HashMap;

use once_cell::sync::Lazy;

// Define the constant HashMap using once_cell
static MY_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
  let mut map = HashMap::new();
  map.insert("key1", "value1");
  map.insert("key2", "value2");
  map
});

fn main() {
  // Access the constant HashMap
  println!("Value for key1: {}", MY_MAP.get("key1").unwrap());
  println!("Value for key2: {}", MY_MAP.get("key2").unwrap());
}
