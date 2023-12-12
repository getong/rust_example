use std::cell::RefCell;
use std::rc::Rc;

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

pub struct Solution;

impl Solution {
  pub fn preorder_traversal(root: Option<Rc<RefCell<TreeNode>>>) -> Vec<i32> {
    let mut result: Vec<i32> = vec![];
    if root.is_none() {
      return result;
    }
    Self::preorder_recursive(root, &mut result);
    result
  }

  pub fn preorder_recursive(root: Option<Rc<RefCell<TreeNode>>>, result: &mut Vec<i32>) {
    match root {
      Some(node) => {
        result.push(node.borrow().val);
        Self::preorder_recursive(node.borrow().left.clone(), result);
        Self::preorder_recursive(node.borrow().right.clone(), result);
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
