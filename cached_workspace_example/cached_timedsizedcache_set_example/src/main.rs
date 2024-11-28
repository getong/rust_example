use std::time::Duration;

use cached::{stores::TimedSizedCache, Cached};

fn main() {
  // Simulate a HashSet using a cache (only care about the keys, not the values)
  let mut data_set: TimedSizedCache<String, ()> = TimedSizedCache::with_size_and_lifespan(3, 5);

  // Insert values into the "HashSet"
  data_set.cache_set("a".to_string(), ());
  data_set.cache_set("b".to_string(), ());
  data_set.cache_set("c".to_string(), ());

  // Check if the set contains a specific key
  println!("Contains 'a': {:?}", data_set.cache_get("a").is_some());

  // Simulate eviction based on size limit
  data_set.cache_set("d".to_string(), ());

  // At this point, "a" might be evicted due to size constraints
  println!(
    "Contains 'a' after inserting 'd': {:?}",
    data_set.cache_get("a").is_some()
  );

  // Wait for the cache to expire (6 seconds)
  std::thread::sleep(Duration::new(6, 0));

  // After sleep, keys should have expired due to time-to-live
  println!(
    "Contains 'b' after sleep: {:?}",
    data_set.cache_get("b").is_some()
  );
}
