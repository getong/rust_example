use masstree::MassTree;

fn main() {
  let tree: MassTree<u64> = MassTree::new();
  let guard = tree.guard();

  // Insert
  tree.insert_with_guard(b"hello", 123, &guard).unwrap();
  tree.insert_with_guard(b"world", 456, &guard).unwrap();

  // Point lookup
  assert_eq!(tree.get_ref(b"hello", &guard), Some(&123));

  // Remove
  tree.remove_with_guard(b"hello", &guard).unwrap();
  assert_eq!(tree.get_ref(b"hello", &guard), None);

  // Range scan (zero-copy)
  tree.scan_ref(
    b"a" .. b"z",
    |key, value| {
      println!("{:?} -> {}", key, value);
      true // continue scanning
    },
    &guard,
  );

  // Prefix scan
  tree.scan_prefix(
    b"wor",
    |key, value| {
      println!("{:?} -> {}", key, value);
      true
    },
    &guard,
  );
}
