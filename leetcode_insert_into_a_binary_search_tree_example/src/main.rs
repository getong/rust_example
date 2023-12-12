#[derive(Debug, PartialEq, Eq)]
pub struct TreeNode {
  pub val: i32,
  pub left: Option<Rc<RefCell<TreeNode>>>,
  pub right: Option<Rc<RefCell<TreeNode>>>,
}

impl TreeNode {
  #[inline]
  pub fn new(val: i32) -> Self {
    TreeNode {
      val,
      left: None,
      right: None,
    }
  }
}

use std::cell::RefCell;
use std::rc::Rc;

pub struct Solution;

impl Solution {
  pub fn insert_into_bst(
    root: Option<Rc<RefCell<TreeNode>>>,
    val: i32,
  ) -> Option<Rc<RefCell<TreeNode>>> {
    if root.is_none() {
      return Some(Rc::new(RefCell::new(TreeNode::new(val))));
    }
    Self::insert(&root, val);
    root
  }

  fn insert(root: &Option<Rc<RefCell<TreeNode>>>, val: i32) {
    if let Some(node) = root {
      let mut n = node.borrow_mut();
      let target = if val > n.val {
        &mut n.right
      } else {
        &mut n.left
      };
      if target.is_some() {
        return Self::insert(target, val);
      }
      *target = Some(Rc::new(RefCell::new(TreeNode::new(val))))
    };
  }
}

fn main() {
  println!("Hello, world!");
}
