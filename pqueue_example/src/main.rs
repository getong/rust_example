fn main() {
    // println!("Hello, world!");
    let items = [9, 5, 1, 3, 4, 2, 6, 8, 9, 2, 1];
    let mut q = pqueue::Queue::new();

    for item in items {
        q.push(item);
    }

    while let Some(item) = q.pop() {
        println!("{}", item);
    }

    // OUTPUT:
    // 1
    // 1
    // 2
    // 2
    // 3
    // 4
    // 5
    // 6
    // 8
    // 9
    // 9
}
