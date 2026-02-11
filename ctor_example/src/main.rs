use std::sync::atomic::{AtomicBool, Ordering};
use ctor::ctor;

static INITED: AtomicBool = AtomicBool::new(false);

#[ctor]
fn foo() {
  INITED.store(true, Ordering::SeqCst);
}

fn main() {
  let is_inited = INITED.load(Ordering::SeqCst);
  println!("INITED: {}", is_inited);
}
