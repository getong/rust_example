use std::{
  sync::{
    atomic::{AtomicBool, Ordering::SeqCst},
    Arc,
  },
  thread,
  time::Duration,
};

use crossbeam_utils::Backoff;

fn spin_wait(ready: &AtomicBool) {
  let backoff = Backoff::new();
  let mut attempt_count = 0;
  while !ready.load(SeqCst) {
    backoff.snooze();
    attempt_count = attempt_count + 1;
    println!("{}, {:?}", attempt_count, backoff);
  }
}

fn main() {
  // println!("Hello, world!");
  let ready = Arc::new(AtomicBool::new(false));
  let ready2 = ready.clone();

  thread::spawn(move || {
    thread::sleep(Duration::from_millis(100));
    ready2.store(true, SeqCst);
  });

  assert_eq!(ready.load(SeqCst), false);
  spin_wait(&ready);
  assert_eq!(ready.load(SeqCst), true);
}
