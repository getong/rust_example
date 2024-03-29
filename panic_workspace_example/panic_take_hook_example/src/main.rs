#![allow(unused)]
fn main() {
  use std::panic;

  panic::set_hook(Box::new(|_| {
    println!("Custom panic hook");
  }));

  let _ = panic::take_hook();

  panic!("Normal panic");
}
