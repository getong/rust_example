use std::time::Duration;

use cached::{Cached, stores::LruTtlCache};

fn main() {
  // Keep up to 3 entries with a 4-second TTL, refreshed on cache hits.
  let mut data_set: LruTtlCache<String, String> = LruTtlCache::builder()
    .max_size(3)
    .ttl(Duration::from_secs(4))
    .refresh_on_hit(true)
    .build()
    .expect("valid cache configuration");

  // Add an entry to the cache
  data_set.cache_set("a".to_string(), "1".to_string());
  data_set.cache_set("b".to_string(), "2".to_string());
  data_set.cache_set("c".to_string(), "3".to_string());
  println!("a value is {:?}, ", data_set.cache_get("a"));
  data_set.cache_set("a".to_string(), "1".to_string());

  data_set.cache_set("d".to_string(), "4".to_string());

  // Collect keys before iterating
  let keys = data_set.key_order();
  println!("keys: {:?}", keys);

  // Print cache before sleep
  println!("Before sleep:");
  for key in &keys {
    println!("Key: {}, Value: {:?}", key, data_set.cache_get(key));
  }

  // Wait for the cache to expire (6 seconds)
  std::thread::sleep(Duration::new(6, 0));

  // Reuse the previous keys: key_order() excludes expired entries.

  // Print cache after sleep
  println!("After sleep:");
  for key in &keys {
    println!("Key: {}, Value: {:?}", key, data_set.cache_get(key));
  }
}
