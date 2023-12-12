use std::sync::{Arc, RwLock};
use std::thread;

fn main() {
  let lock = Arc::new(RwLock::new(11));
  let c_lock = Arc::clone(&lock);

  let _ = thread::spawn(move || {
    let _lock = c_lock.write().unwrap();
    panic!(); // the lock gets poisoned
  })
  .join();

  let read = match lock.read() {
    Ok(l) => *l,
    Err(poisoned) => {
      let r = poisoned.into_inner();
      *r + 1
    }
  };

  // It will be 12 because it was recovered from the poisoned lock
  assert_eq!(read, 12);
}
