fn main() {
  // println!("Hello, world!");
  let tree = sled::open("/tmp/welcome-to-sled").unwrap();

  // insert and get, similar to std's BTreeMap
  let _old_value = tree.insert("key", "value").unwrap();

  assert_eq!(tree.get(&"key").unwrap(), Some(sled::IVec::from("value")));

  // range queries
  for _kv_result in tree.range("key_1" .. "key_9") {}

  // deletion
  let _old_value = tree.remove(&"key").unwrap();

  // atomic compare and swap
  let _ = tree
    .compare_and_swap("key", Some("current_value"), Some("new_value"))
    .unwrap();

  // block until all operations are stable on disk
  // (flush_async also available to get a Future)
  let _ = tree.flush().unwrap();
}
