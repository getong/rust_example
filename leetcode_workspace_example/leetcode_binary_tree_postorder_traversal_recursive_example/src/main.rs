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
  pub fn postorder_traversal(root: Option<Rc<RefCell<TreeNode>>>) -> Vec<i32> {
    let mut result: Vec<i32> = vec![];
    if root.is_none() {
      return result;
    }

    Self::postorder_recursive(root, &mut result);
    result
  }

  pub fn postorder_recursive(root: Option<Rc<RefCell<TreeNode>>>, result: &mut Vec<i32>) {
    match root {
      Some(node) => {
        Self::postorder_recursive(node.borrow().left.clone(), result);
        Self::postorder_recursive(node.borrow().right.clone(), result);
        result.push(node.borrow().val);
      }
      None => {
        return;
      }
    }
  }
}

fn main() {
  println!("Hello, world!");
}
