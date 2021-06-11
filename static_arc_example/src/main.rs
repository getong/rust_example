use lazy_static::lazy_static;

use std::sync::{Arc, Mutex};

lazy_static! {
    // static ref CONTEXT = Rc:new();
    static ref PRESSED : Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
}

fn main() {
    // println!("Hello, world!");
    println!("PRESSED: {}", PRESSED.lock().unwrap());
}
