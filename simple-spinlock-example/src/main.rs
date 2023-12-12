use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::{hint, thread};

fn main() {
  let spinlock = Arc::new(AtomicUsize::new(1));

  let spinlock_clone = Arc::clone(&spinlock);
  let thread = thread::spawn(move || {
    spinlock_clone.store(0, Ordering::SeqCst);
  });

  // Wait for the other thread to release the lock
  while spinlock.load(Ordering::SeqCst) != 0 {
    hint::spin_loop();
  }

  if let Err(panic) = thread.join() {
    println!("Thread had an error: {:?}", panic);
  }
}
