use cached::{stores::TimedSizedCache, Cached};
use std::time::Duration;

fn main() {
  // Create a TimedSizedCache with a size of 1000, lifespan of 5 seconds, and refresh enabled
  let mut data_set: TimedSizedCache<String, String> =
    TimedSizedCache::with_size_and_lifespan_and_refresh(3, 4, true);

  // Add an entry to the cache
  data_set.cache_set("a".to_string(), "1".to_string());
  data_set.cache_set("b".to_string(), "2".to_string());
  data_set.cache_set("c".to_string(), "3".to_string());
  println!("a value is {:?}, ", data_set.cache_get("a"));
  data_set.cache_set("a".to_string(), "1".to_string());

  data_set.cache_set("d".to_string(), "4".to_string());

  // Collect keys before iterating
  let keys: Vec<_> = data_set.key_order().cloned().collect();
  println!("keys: {:?}", keys);

  // Print cache before sleep
  println!("Before sleep:");
  for key in &keys {
    println!("Key: {}, Value: {:?}", key, data_set.cache_get(key));
  }

  // Wait for the cache to expire (6 seconds)
  std::thread::sleep(Duration::new(6, 0));

  // Collect keys again before iterating
  let keys: Vec<_> = data_set.key_order().cloned().collect();

  // Print cache after sleep
  println!("After sleep:");
  for key in &keys {
    println!("Key: {}, Value: {:?}", key, data_set.cache_get(key));
  }
}
