use std::thread;
use trc::SharedTrc;
use trc::Trc;
use trc::Weak;

fn basic_trc() {
  let mut trc = Trc::new(100);
  assert_eq!(*trc, 100);
  let mutref = unsafe { trc.deref_mut() };
  *mutref = 200;
  assert_eq!(*trc, 200);
}

fn multiple_thread_trc() {
  let trc = Trc::new(100);
  let shared = SharedTrc::from_trc(&trc);
  let handle = thread::spawn(move || {
    let mut trc = SharedTrc::to_trc(shared);
    *unsafe { Trc::deref_mut(&mut trc) } = 200;
  });

  handle.join().unwrap();
  assert_eq!(*trc, 200);
}

fn weak_trc() {
  let trc = Trc::new(100);
  let weak = Weak::from_trc(&trc);
  let mut new_trc = Weak::to_trc(&weak).unwrap();
  println!("Deref test! {}", *new_trc);
  println!("DerefMut test");
  *unsafe { Trc::deref_mut(&mut new_trc) } = 200;
  println!("Deref test! {}", *new_trc);
}

fn multiple_weak_trc() {
  let trc = Trc::new(100);
  let weak = Weak::from_trc(&trc);

  let handle = thread::spawn(move || {
    let mut trc = Weak::to_trc(&weak).unwrap();
    println!("{:?}", *trc);
    *unsafe { Trc::deref_mut(&mut trc) } = 200;
  });
  handle.join().unwrap();
  println!("{}", *trc);
  assert_eq!(*trc, 200);
}

fn main() {
  // println!("Hello, world!");
  basic_trc();
  multiple_thread_trc();
  weak_trc();
  multiple_weak_trc();
}
