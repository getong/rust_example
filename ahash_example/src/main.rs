use ahash::AHashMap;

fn main() {
  // println!("Hello, world!");
  let mut map: AHashMap<i32, i32> = AHashMap::new();
  map.insert(12, 34);
  map.insert(56, 78);
  println!("map: {:?}", map);
}
