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
    pub fn preorder_traversal(root: Option<Rc<RefCell<TreeNode>>>) -> Vec<i32> {
        let mut result = vec![];
        if root.is_none() {
            return result;
        }

        let mut stack: Vec<Rc<RefCell<TreeNode>>> = Vec::new();
        let mut r = root.clone();

        while r.is_some() || !stack.is_empty() {
            while let Some(node) = r {
                result.push(node.borrow().val);
                stack.push(node.clone());
                r = node.borrow().left.clone();
            }

            r = stack.pop();
            if let Some(node) = r {
                r = node.borrow().right.clone();
            }
        }
        result
    }
}

fn main() {
    println!("Hello, world!");
}
