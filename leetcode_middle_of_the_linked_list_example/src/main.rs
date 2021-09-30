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
    pub fn middle_node(head: Option<Box<ListNode>>) -> Option<Box<ListNode>> {
        let mut fast_p = &head;
        let mut slow_p = &head;

        while fast_p.is_some() && fast_p.as_ref().unwrap().next.is_some() {
            slow_p = &slow_p.as_ref().unwrap().next;
            fast_p = &fast_p.as_ref().unwrap().next.as_ref().unwrap().next;
        }
        slow_p.clone()
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
    let r3 = l3.clone();
    l2.next = Some(Box::new(l3));
    l1.next = Some(Box::new(l2));

    assert_eq!(
        Solution::middle_node(Some(Box::new(l1))),
        Some(Box::new(r3))
    );
}
