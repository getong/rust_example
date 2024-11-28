use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;

lazy_static! {
    // static ref CONTEXT = Rc:new();
    static ref PRESSED : Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
}

fn main() {
  // println!("Hello, world!");
  println!("PRESSED: {}", PRESSED.lock().unwrap());
}
