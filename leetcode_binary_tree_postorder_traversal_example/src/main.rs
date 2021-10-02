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
    pub fn postorder_traversal(root: Option<Rc<RefCell<TreeNode>>>) -> Vec<i32> {
        let mut result = vec![];
        if root.is_none() {
            return result;
        }

        let mut stack1: Vec<Option<Rc<RefCell<TreeNode>>>> = vec![];
        let mut stack2: Vec<Option<Rc<RefCell<TreeNode>>>> = vec![];
        stack1.push(root);
        while let Some(Some(node)) = stack1.pop() {
            if node.borrow().left.is_some() {
                stack1.push(node.borrow().left.clone());
            }
            if node.borrow().right.is_some() {
                stack1.push(node.borrow().right.clone());
            }
            stack2.push(Some(node));
        }

        while let Some(Some(node)) = stack2.pop() {
            result.push(node.borrow().val)
        }
        result
    }
}

fn main() {
    println!("Hello, world!");
}
