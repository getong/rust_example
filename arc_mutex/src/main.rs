// arc_mutex.rs
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

struct MySharedData {
  pub counter: u32,
}

impl MySharedData {
  pub fn new() -> MySharedData {
    MySharedData { counter: 0 }
  }
}

fn spawn_threads() {
  let shared_data = Arc::new(Mutex::new(MySharedData::new()));
  // Spawn a number of threads and collect their join handles
  let handles: Vec<JoinHandle<_>> = (0..10)
    .map(|_| {
      let shared_data = shared_data.clone();
      thread::spawn(move || {
        let mut shared_data = shared_data.lock().unwrap();
        shared_data.counter += 1;
      })
    })
    .collect();
  // Wait for each thread to complete
  for h in handles {
    _ = h.join();
  }
  // Print the data
  let shared_data = shared_data.lock().unwrap();
  println!("Total = {}", shared_data.counter);
}

fn main() {
  let vec = Arc::new(Mutex::new(vec![]));
  let mut childs = vec![];
  for i in 0..5 {
    let v = vec.clone();
    let t = thread::spawn(move || {
      let mut v = v.lock().unwrap();
      v.push(i);
    });
    childs.push(t);
  }

  for c in childs {
    c.join().unwrap();
  }

  println!("{:?}", vec);

  let data = Arc::new(Mutex::new(0));
  for _ in 0..15 {
    let data = Arc::clone(&data);
    thread::spawn(move || {
      let mut data = data.lock().unwrap();
      *data += 1;
      if *data == 15 {
        return;
      }
    });
  }
  println!("data: {}", data.lock().unwrap());

  println!("----------------");

  spawn_threads();
}
