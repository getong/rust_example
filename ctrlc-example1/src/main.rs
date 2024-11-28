use std::sync::{
  atomic::{AtomicBool, Ordering},
  Arc,
};

fn main() {
  let running = Arc::new(AtomicBool::new(true));
  let r = running.clone();

  ctrlc::set_handler(move || {
    r.store(false, Ordering::SeqCst);
  })
  .expect("Error setting Ctrl-C handler");

  println!("Waiting for Ctrl-C...");
  while running.load(Ordering::SeqCst) {}
  println!("Got it! Exiting...");
}
