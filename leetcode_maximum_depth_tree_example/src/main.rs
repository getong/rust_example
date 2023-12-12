// Definition for a binary tree node.
// #[derive(Debug, PartialEq, Eq)]
// pub struct TreeNode {
//   pub val: i32,
//   pub left: Option<Rc<RefCell<TreeNode>>>,
//   pub right: Option<Rc<RefCell<TreeNode>>>,
// }
//
// impl TreeNode {
//   #[inline]
//   pub fn new(val: i32) -> Self {
//     TreeNode {
//       val,
//       left: None,
//       right: None
//     }
//   }
// }
use std::cell::RefCell;
use std::rc::Rc;
impl Solution {
  pub fn max_depth(root: Option<Rc<RefCell<TreeNode>>>) -> i32 {
    Self::next_node(root.as_ref(), 0)
  }

  fn next_node(node: Option<&Rc<RefCell<TreeNode>>>, mut num: i32) -> i32 {
    match node {
      None => num,
      Some(rc_node) => {
        num += 1;
        let lmax = Self::next_node(rc_node.borrow().left.as_ref(), num);
        let rmax = Self::next_node(rc_node.borrow().right.as_ref(), num);
        lmax.max(rmax)
      }
    }
  }
}

fn main() {
  println!("Hello, world!");
}
