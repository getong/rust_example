use petgraph::dot::Dot;
use petgraph::graph::Graph;
use std::fs;
use std::process::Command;

fn main() -> std::io::Result<()> {
  let mut g: Graph<&str, &str> = Graph::new();

  let a = g.add_node("Node A");
  let b = g.add_node("Node B");
  g.add_edge(a, b, "edge A->B");

  fs::write("graph.dot", format!("{}", Dot::new(&g)))?;

  Command::new("dot")
    .args(["-Tpng", "graph.dot", "-o", "graph.png"])
    .status()?;

  Ok(())
}
