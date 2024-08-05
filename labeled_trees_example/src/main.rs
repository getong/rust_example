use std::collections::HashSet;

#[derive(PartialEq, Eq, Hash, Debug)]
struct Edge(u32, u32);

type Tree = HashSet<Edge>;

fn create_tree_from_seq(mut s: &[u32], n: u32) -> Tree {
  let mut res = Tree::new();
  let mut l: Vec<u32> = (1 .. n + 1).filter(|e| !s.contains(e)).collect();
  l.sort();
  while s.len() > 0 {
    let v = s[0];
    s = &s[1 ..];
    let l1 = l.remove(0);
    res.insert(Edge(v, l1));
    if !s.contains(&v) {
      l.push(v);
      l.sort();
    }
  }
  res.insert(Edge(l[0], l[1]));
  res
}

fn rec_find_sequences(n: u32, len: u32) -> Vec<Vec<u32>> {
  if len == 0 {
    vec![vec![]]
  } else {
    let mut res = vec![];
    let sqs = rec_find_sequences(n, len - 1);
    for i in 1 .. n + 1 {
      for sq in sqs.iter() {
        let mut sq_clone = sq.clone();
        sq_clone.insert(0, i);
        res.push(sq_clone);
      }
    }
    res
  }
}

fn find_all_labeled_trees(n: u32) -> Vec<Tree> {
  rec_find_sequences(n, n - 2)
    .iter()
    .map(|seq| create_tree_from_seq(seq, n))
    .collect()
}

fn main() {
  println!("{:?}", find_all_labeled_trees(5));
}
