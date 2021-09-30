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
    pub fn reverse_list(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
        let mut prev = None;
        let mut curr = head;

        while let Some(mut curr_node) = curr.take() {
            let next_temp = curr_node.next.take();
            curr_node.next = prev.take();
            prev = Some(curr_node);
            curr = next_temp;
        }
        prev
    }
}

fn main() {
    // println!("Hello, world!");
    let mut l1 = ListNode::new(1);
    let mut l2 = ListNode::new(2);
    let mut l3 = ListNode::new(3);
    let mut l4 = ListNode::new(4);
    let l5 = ListNode::new(5);
    l4.next = Some(Box::new(l5));
    l3.next = Some(Box::new(l4));
    l2.next = Some(Box::new(l3));
    l1.next = Some(Box::new(l2));

    let r1 = ListNode::new(1);
    let mut r2 = ListNode::new(2);
    let mut r3 = ListNode::new(3);
    let mut r4 = ListNode::new(4);
    let mut r5 = ListNode::new(5);
    r2.next = Some(Box::new(r1));
    r3.next = Some(Box::new(r2));
    r4.next = Some(Box::new(r3));
    r5.next = Some(Box::new(r4));

    assert_eq!(
        Solution::reverse_list(Some(Box::new(l1))),
        Some(Box::new(r5))
    );
}
