use std::thread;

use trc::{SharedTrc, Trc, Weak};

fn basic_trc() {
  let mut trc = Trc::new(100);
  assert_eq!(*trc, 100);
  *Trc::get_mut(&mut trc).unwrap() = 200;
  assert_eq!(*trc, 200);
}

fn multiple_thread_trc() {
  let trc = Trc::new(100);
  let shared = SharedTrc::from_trc(&trc);
  let handle = thread::spawn(move || {
    let _trc = SharedTrc::to_trc(shared);
  });

  handle.join().unwrap();
  assert_eq!(*trc, 100);
}

fn weak_trc() {
  let trc = Trc::new(100);
  let weak = Trc::downgrade(&trc);
  let new_trc = Weak::upgrade(&weak).unwrap();
  assert_eq!(*new_trc, 100);
}

fn multiple_weak_trc() {
  let trc = Trc::new(100);
  let shared = SharedTrc::from_trc(&trc);
  let handle = thread::spawn(move || {
    let trc = SharedTrc::to_trc(shared);
    assert_eq!(*trc, 100);
  });
  handle.join().unwrap();
  assert_eq!(*trc, 100);
}

fn main() {
  // println!("Hello, world!");
  basic_trc();
  multiple_thread_trc();
  weak_trc();
  multiple_weak_trc();
}
