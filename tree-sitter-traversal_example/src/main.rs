use std::collections::HashSet;

use tree_sitter::{Node, Tree};
use tree_sitter_traversal::{traverse, traverse_tree, Order};

fn get_tree() -> Tree {
  use tree_sitter::Parser;
  let mut parser = Parser::new();
  let lang = tree_sitter_rust::language();
  parser
    .set_language(lang)
    .expect("Error loading Rust grammar");
  return parser
    .parse("fn double(x: usize) -> usize { x * 2 }", None)
    .expect("Error parsing provided code");
}

fn main() {
  let tree: Tree = get_tree();
  let preorder: Vec<Node<'_>> = traverse(tree.walk(), Order::Pre).collect::<Vec<_>>();
  let postorder: Vec<Node<'_>> = traverse_tree(&tree, Order::Post).collect::<Vec<_>>();
  // For any tree with more than just a root node,
  // the order of preorder and postorder will be different
  assert_ne!(preorder, postorder);
  // However, they will have the same amount of nodes
  assert_eq!(preorder.len(), postorder.len());
  // Specifically, they will have the exact same nodes, just in a different order
  assert_eq!(
    <HashSet<_>>::from_iter(preorder.into_iter()),
    <HashSet<_>>::from_iter(postorder.into_iter())
  );
}
