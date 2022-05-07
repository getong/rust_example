use priority_queue::PriorityQueue;

fn main() {
    // println!("Hello, world!");
    let mut pq = PriorityQueue::new();

    assert!(pq.is_empty());
    pq.push("Apples", 5);
    pq.push("Bananas", 8);
    pq.push("Strawberries", 23);

    assert_eq!(pq.peek(), Some((&"Strawberries", &23)));

    pq.change_priority("Bananas", 25);
    assert_eq!(pq.peek(), Some((&"Bananas", &25)));

    for (item, num) in pq.into_sorted_iter() {
        println!("{}: {}", item, num);
    }
}
