use std::fs;

use graphviz_rust::{cmd::Format, exec_dot};
use petgraph::{dot::Dot, graph::Graph};

fn main() -> std::io::Result<()> {
  let mut g: Graph<&str, &str> = Graph::new();

  let a = g.add_node("Node A");
  let b = g.add_node("Node B");
  g.add_edge(a, b, "edge A->B");

  let dot = Dot::new(&g).to_string();
  fs::write("graph.dot", &dot)?;

  let png = exec_dot(dot, vec![Format::Png.into()])?;
  fs::write("graph.png", png)?;

  Ok(())
}
