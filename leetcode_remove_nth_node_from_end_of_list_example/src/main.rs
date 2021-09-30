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
    pub fn remove_nth_from_end(head: Option<Box<ListNode>>, n: i32) -> Option<Box<ListNode>> {
        let mut dummy = Some(Box::new(ListNode { val: 0, next: head }));
        let mut slow_p = &mut dummy;
        let mut fast_p = &mut slow_p.clone();

        for _ in 1..=n + 1 {
            fast_p = &mut fast_p.as_mut().unwrap().next;
        }

        while fast_p.is_some() {
            fast_p = &mut fast_p.as_mut().unwrap().next;
            slow_p = &mut slow_p.as_mut().unwrap().next;
        }

        let next = &slow_p.as_mut().unwrap().next.as_mut().unwrap().next;
        slow_p.as_mut().unwrap().next = next.clone();
        dummy.unwrap().next
    }
}

fn main() {
    // println!("Hello, world!");
    let mut l1 = ListNode::new(1);
    let mut l2 = ListNode::new(2);
    let mut l3 = ListNode::new(3);
    let mut l4 = ListNode::new(4);
    let l5 = ListNode::new(5);

    let mut r1 = l1.clone();
    let mut r2 = l2.clone();
    let mut r3 = l3.clone();
    let r5 = l5.clone();
    r3.next = Some(Box::new(r5));
    r2.next = Some(Box::new(r3));
    r1.next = Some(Box::new(r2));

    l4.next = Some(Box::new(l5));
    l3.next = Some(Box::new(l4));

    l2.next = Some(Box::new(l3));
    l1.next = Some(Box::new(l2));

    assert_eq!(
        Solution::remove_nth_from_end(Some(Box::new(l1)), 2),
        Some(Box::new(r1))
    );
}
