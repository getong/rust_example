use std::sync::{Arc, Barrier};
use std::thread;

fn main() {
    // println!("Hello, world!");
    let mut handles = Vec::with_capacity(5);
    let barrier = Arc::new(Barrier::new(5));

    for _ in 0..5 {
        let c = barrier.clone();
        handles.push(thread::spawn(move || {
            println!("before wait");
            c.wait();
            println!("after wait");
        }));
    }
    for handle in handles {
        handle.join().unwrap();
    }
}
