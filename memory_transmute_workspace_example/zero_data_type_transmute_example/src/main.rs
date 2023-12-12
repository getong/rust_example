use std::mem::transmute;

fn main() {
  let x: Box<()> = Box::new(());
  let y: Option<Box<()>> = None;
  let z: Option<Box<()>> = Some(Box::new(()));

  unsafe {
    let value1: usize = transmute(x);
    let value2: usize = transmute(y);
    let value3: usize = transmute(z);
    println!("{} {} {}", value1, value2, value3);
  }
}
