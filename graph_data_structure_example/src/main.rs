use std::collections::{HashMap, HashSet};

pub type Graph = HashMap<u32, HashSet<u32>>;

pub fn connected_components(g: &Graph) -> Vec<Graph> {
  let mut non_checked_vertexes = g.keys().map(|e| *e).collect::<HashSet<u32>>();
  let mut res = vec![];
  while non_checked_vertexes.len() > 0 {
    let mut component = Graph::new();
    fill_component(
      *non_checked_vertexes.iter().next().unwrap(),
      &mut component,
      g,
      &mut non_checked_vertexes,
    );
    res.push(component);
  }
  res
}

fn fill_component(
  v: u32,
  component: &mut Graph,
  g: &Graph,
  non_checked_vertexes: &mut HashSet<u32>,
) {
  add_connections_to_component(v, component, g, non_checked_vertexes);
  let neighbors = g.get(&v).unwrap();
  while !neighbors.is_disjoint(&non_checked_vertexes) {
    let nv = *neighbors
      .intersection(&non_checked_vertexes)
      .next()
      .unwrap();
    fill_component(nv, component, g, non_checked_vertexes);
  }
}

fn add_connections_to_component(
  v: u32,
  component: &mut Graph,
  g: &Graph,
  non_checked_vertexes: &mut HashSet<u32>,
) {
  let neighbors = g.get(&v).unwrap();
  component.insert(v, neighbors.clone());
  non_checked_vertexes.remove(&v);
}

fn main() {
  let g = Graph::from([
    (1, HashSet::from([2, 4, 5])),
    (2, HashSet::from([1, 3])),
    (3, HashSet::from([2, 5, 4])),
    (4, HashSet::from([1, 3])),
    (5, HashSet::from([1, 3])),
    (6, HashSet::from([7])),
    (7, HashSet::from([6, 8, 9])),
    (8, HashSet::from([7, 9])),
    (9, HashSet::from([7, 8])),
  ]);
  for c in connected_components(&g) {
    println!("{:?}", c);
  }
}
