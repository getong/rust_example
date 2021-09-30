#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ListNode {
    pub val: i32,
    pub next: Option<Box<ListNode>>,
}

impl ListNode {
    #[inline]
    fn new(val: i32) -> Self {
        ListNode { next: None, val }
    }
}

struct Solution;

impl Solution {
    pub fn merge_two_lists(
        listnode1: Option<Box<ListNode>>,
        listnode2: Option<Box<ListNode>>,
    ) -> Option<Box<ListNode>> {
        match (listnode1, listnode2) {
            (Some(node1), None) => Some(node1),
            (None, Some(node2)) => Some(node2),

            (Some(mut node1), Some(mut node2)) => {
                if node1.val < node2.val {
                    let n = node1.next.take();
                    node1.next = Self::merge_two_lists(n, Some(node2));
                    Some(node1)
                } else {
                    let n = node2.next.take();
                    node2.next = Self::merge_two_lists(Some(node1), n);
                    Some(node2)
                }
            }

            _ => None,
        }
    }
}

fn main() {
    // println!("Hello, world!");

    let mut l1 = ListNode::new(1);
    let mut l2 = ListNode::new(2);
    let l4 = ListNode::new(4);
    l2.next = Some(Box::new(l4));
    l1.next = Some(Box::new(l2));

    let mut r1 = ListNode::new(1);
    let mut r3 = ListNode::new(3);
    let r4 = ListNode::new(4);
    r3.next = Some(Box::new(r4));
    r1.next = Some(Box::new(r3));

    let mut result1 = ListNode::new(1);
    let mut result2 = ListNode::new(2);
    let mut result3 = ListNode::new(3);
    let mut result4 = ListNode::new(4);
    result4.next = Some(Box::new(result4.clone()));
    result3.next = Some(Box::new(result4));
    result2.next = Some(Box::new(result3));
    result1.next = Some(Box::new(result2));
    result1.next = Some(Box::new(result1.clone()));
    assert_eq!(
        Solution::merge_two_lists(Some(Box::new(l1)), Some(Box::new(r1))),
        Some(Box::new(result1))
    );
}
