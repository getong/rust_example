use petgraph::{algo::dijkstra, dot::Dot, prelude::*};

fn main() {
  // println!("Hello, world!");
  // let root = TypedArena::<Node<_>>::new();
  let mut gr = UnGraphMap::new();
  // let node = |&: name: &'static str| Ptr(root.alloc(Node(name.to_string())));
  let a = gr.add_node("A");
  let b = gr.add_node("B");
  let c = gr.add_node("C");
  let d = gr.add_node("D");
  let e = gr.add_node("E");
  let f = gr.add_node("F");
  gr.add_edge(a, b, 7);
  gr.add_edge(a, c, 9);
  gr.add_edge(a, d, 14);
  gr.add_edge(b, c, 10);
  gr.add_edge(c, d, 2);
  gr.add_edge(d, e, 9);
  gr.add_edge(b, f, 15);
  gr.add_edge(c, f, 11);

  assert!(gr.add_edge(e, f, 5).is_none());

  // duplicate edges
  assert_eq!(gr.add_edge(f, b, 16), Some(15));
  assert_eq!(gr.add_edge(f, e, 6), Some(5));
  println!("{:?}", gr);
  println!("{}", Dot::with_config(&gr, &[]));

  assert_eq!(gr.node_count(), 6);
  assert_eq!(gr.edge_count(), 9);

  // check updated edge weight
  assert_eq!(gr.edge_weight(e, f), Some(&6));
  let scores = dijkstra(&gr, a, None, |e| *e.weight());
  let mut scores: Vec<_> = scores.into_iter().collect();
  scores.sort();
  assert_eq!(
    scores,
    vec![
      ("A", 0),
      ("B", 7),
      ("C", 9),
      ("D", 11),
      ("E", 20),
      ("F", 20)
    ]
  );
}
