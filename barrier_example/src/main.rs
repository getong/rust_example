use std::sync::{Arc, Barrier};
use std::thread;

fn main() {
  let mut handles = Vec::with_capacity(5);
  let barrier = Arc::new(Barrier::new(5));

  for _ in 0..5 {
    // let c = Arc::clone(&barrier);
    let c = barrier.clone();

    // The same messages will be printed together.
    // You will NOT see any interleaving.
    handles.push(thread::spawn(move || {
      println!("before wait");
      c.wait();
      println!("after wait");
    }));
  }

  // Wait for other threads to finish.
  for handle in handles {
    handle.join().unwrap();
  }
}
