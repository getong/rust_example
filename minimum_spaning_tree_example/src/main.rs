use std::{
  collections::{HashMap, HashSet},
  hash::{Hash, Hasher},
};

#[derive(Debug, Clone)]
struct Edge(u32, u32);

impl Edge {
  fn is_end(&self, v: u32) -> bool {
    self.0 == v || self.1 == v
  }
}

impl PartialEq for Edge {
  fn eq(&self, other: &Self) -> bool {
    (self.0, self.1) == (other.0, other.1) || (self.0, self.1) == (other.1, other.0)
  }
}

impl Eq for Edge {}

impl Hash for Edge {
  fn hash<H: Hasher>(&self, state: &mut H) {
    if self.0 < self.1 {
      self.0.hash(state);
      self.1.hash(state);
    } else {
      self.1.hash(state);
      self.0.hash(state);
    }
  }
}

type Tree = HashSet<Edge>;
type Graph = HashMap<Edge, u32>;

fn is_vertex_in_tree(t: &Tree, v: u32) -> bool {
  t.iter().any(|e| e.is_end(v))
}

fn combine_trees(mut trees: Vec<Tree>, Edge(u, v): &Edge) -> Vec<Tree> {
  let mut res = vec![];
  let mut combined = Tree::from([Edge(*u, *v)]);
  loop {
    if let Some(tree) = trees.pop() {
      if is_vertex_in_tree(&tree, *v) || is_vertex_in_tree(&tree, *u) {
        combined.extend(tree.into_iter());
      } else {
        res.push(tree);
      }
    } else {
      res.push(combined);
      return res;
    }
  }
}

fn min_spanning_tree(g: &Graph) -> Tree {
  let mut trees: Vec<Tree> = vec![];
  loop {
    if let Some((e, ..)) = g
      .iter()
      .filter(|(Edge(v, u), ..)| {
        trees
          .iter()
          .all(|t| !is_vertex_in_tree(t, *v) || !is_vertex_in_tree(t, *u))
      })
      .min_by_key(|(_, weight)| *weight)
    {
      trees = combine_trees(trees, e);
    } else {
      return trees.pop().unwrap();
    }
  }
}

fn main() {
  let g = Graph::from([
    (Edge(1, 2), 7),
    (Edge(2, 3), 6),
    (Edge(3, 4), 4),
    (Edge(4, 5), 7),
    (Edge(5, 6), 2),
    (Edge(6, 1), 5),
    (Edge(1, 7), 6),
    (Edge(2, 7), 5),
    (Edge(3, 7), 2),
    (Edge(4, 7), 3),
    (Edge(5, 7), 4),
    (Edge(6, 7), 3),
  ]);
  println!("{:?}", min_spanning_tree(&g));
}
