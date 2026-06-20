use std::fmt;

use petgraph::{
  Direction,
  algo::dijkstra,
  dot::Dot,
  graph::{NodeIndex, UnGraph},
  graphmap::UnGraphMap,
  visit::EdgeRef,
};

fn run_graphmap_dijkstra_example() {
  // println!("Hello, world!");
  // let root = TypedArena::<Node<_>>::new();
  let mut gr: UnGraphMap<&str, i32> = UnGraphMap::new();
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
  let scores = dijkstra(&gr, a, None, |edge| *edge.weight());
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

#[derive(Debug)]
struct Fighter {
  name: String,
}

// This is a bit like the following Python code:
//
// class Fighter:
// def __init__(self, name):
// self.name = name
impl Fighter {
  fn new(name: &str) -> Self {
    Self {
      name: name.to_string(),
    }
  }
}

impl fmt::Display for Fighter {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(f, "{}", self.name)
  }
}

fn add_edge(graph: &mut UnGraph<&Fighter, f32>, nodes: &[NodeIndex], a: usize, b: usize) {
  graph.add_edge(nodes[a], nodes[b], 1.0);
}

fn calculate_fighter_centrality() {
  let mut graph = UnGraph::new_undirected();

  let fighters = [
    Fighter::new("Dustin Poirier"),
    Fighter::new("Khabib Nurmagomedov"),
    Fighter::new("Jose Aldo"),
    Fighter::new("Conor McGregor"),
    Fighter::new("Nate Diaz"),
  ];

  let fighter_nodes: Vec<NodeIndex> = fighters
    .iter()
    .map(|fighter| graph.add_node(fighter))
    .collect();

  add_edge(&mut graph, &fighter_nodes, 0, 1); // Dustin Poirier vs. Khabib Nurmagomedov
  add_edge(&mut graph, &fighter_nodes, 1, 3); // Khabib Nurmagomedov vs. Conor McGregor
  add_edge(&mut graph, &fighter_nodes, 3, 0); // Conor McGregor vs. Dustin Poirier
  add_edge(&mut graph, &fighter_nodes, 3, 2); // Conor McGregor vs. Jose Aldo
  add_edge(&mut graph, &fighter_nodes, 3, 4); // Conor McGregor vs. Nate Diaz
  add_edge(&mut graph, &fighter_nodes, 0, 4); // Dustin Poirier vs. Nate Diaz
  add_edge(&mut graph, &fighter_nodes, 2, 4); // Jose Aldo vs. Nate Diaz

  for (i, &node) in fighter_nodes.iter().enumerate() {
    let name = &fighters[i].name;
    let degree = graph.edges_directed(node, Direction::Outgoing).count() as f32;
    let closeness = 1.0 / degree;
    println!("The closeness centrality of {} is {:.2}", name, closeness);

    // Explanation
    match name.as_str() {
      "Conor McGregor" => println!(
        "{} has the lowest centrality because he has fought with all other fighters in the \
         network. In this context, a lower centrality value means a higher number of fights.",
        name
      ),
      "Dustin Poirier" | "Nate Diaz" => println!(
        "{} has a centrality of {:.2}, implying they had less fights compared to Conor McGregor \
         but more than Khabib Nurmagomedov and Jose Aldo.",
        name, closeness
      ),
      "Khabib Nurmagomedov" | "Jose Aldo" => println!(
        "{} has the highest centrality of {:.2} as they have fought with the least number of \
         fighters.",
        name, closeness
      ),
      _ => {}
    }
    println!("-----------------");
  }
}

fn main() {
  run_graphmap_dijkstra_example();
  calculate_fighter_centrality();
}
