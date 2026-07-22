use mpchash::HashRing;

fn main() {
  // Anything that implements `Hash + Send` can be used as a node.
  // Other traits used here are derived for testing purposes.
  #[derive(Hash, Debug, PartialEq, Clone, Copy)]
  struct MyNode(u64);

  // Create a new ring, and add nodes to it.
  let ring = HashRing::new();
  (1 ..= 5).for_each(|i| {
    ring.add(MyNode(i));
  });

  // Anything that implements `Hash` can be used as a key.
  // To find which node should own a key:
  let key = "hello world";

  // Token is a thin wrapper holding reference to node itself
  // and to its position on the ring.
  let token = ring.node(&key).expect("empty ring");
  assert_eq!(token.position(), 1242564280540428107);
  assert_eq!(token.node(), &MyNode(2));

  // In replicated settings, we want to have several replicas
  // of a key to be stored redundantly, therefore we need multiple
  // destination/owning nodes.
  //
  // Assuming a replication factor of 3, we can do:
  let tokens = ring.replicas(&key, 3);
  assert_eq!(tokens, vec![&MyNode(1), &MyNode(2), &MyNode(3)]);

  // Before node removal we probably need to move its data.
  // To find out range of keys owned by a node:
  let ranges = ring.intervals(&token).expect("empty ring");
  assert_eq!(ranges.len(), 1);

  // The range starts at the position where previous node ends,
  // and ends at the position of the owning node.
  assert_eq!(ranges[0].start, ring.position(&MyNode(1)));
  assert_eq!(ranges[0].end, ring.position(&token.node()));

  // Remove node and check the owning nodes again.
  ring.remove(&MyNode(2));

  // `MyNode(2)` is removed, `MyNode(4)` takes its place now.
  let token = ring.node(&key).expect("empty ring");
  assert_eq!(token.node(), &MyNode(4));

  let tokens = ring.replicas(&key, 3);
  assert_eq!(tokens, vec![&MyNode(1), &MyNode(3), &MyNode(4)]);


  // adding new node
  (6 ..= 10).for_each(|i| {
    ring.add(MyNode(i));
  });

  // After adding nodes 6-10, check the new key mapping:
  // Only neighboring keys are affected — that's the core of consistency hashing.
  let token = ring.node(&key).expect("empty ring");
  println!("key \"{}\" now belongs to {:?}", key, token.node());

  let tokens = ring.replicas(&key, 3);
  println!("replication(3) for \"{}\": {:?}", key, tokens);

  // Show all intervals to visualize the ring's key distribution.
  println!("\n--- Ring intervals after adding nodes 6-10 ---");
  for node_id in 1..=10 {
    let t = ring.node(&MyNode(node_id)).unwrap_or_else(|| panic!("node {} not found", node_id));
    let ranges = ring.intervals(&t).expect("empty ring");
    for r in &ranges {
      println!(
        "Node {:?} owns range [{}, {})",
        t.node(),
        r.start,
        r.end,
      );
    }
  }

  // Demonstrate that adding nodes only shifts keys between neighbors:
  // Before: MyNode(2) was removed, MyNode(4) owned "hello world"
  // After:  nodes 6-10 are added, but MyNode(1) still owns "hello world"
  //         because the new nodes don't sit between MyNode(1) and the key.
  //
  // Consistency hashing ensures that when a node is added/removed,
  // only the keys in the affected range are redistributed.
  // The number of affected keys is O(K/N) where K = total keys, N = total nodes.
  println!("\n--- Consistency hashing key insight ---");
  println!("After adding nodes 6-10, only keys in the affected range are remapped.");
  println!("Most keys stay on their original node — minimizing data migration.");
}
