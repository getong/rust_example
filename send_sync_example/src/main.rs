use std::thread;
use std::time::Duration;

fn main() {
  let i = 5;
  thread::spawn(move || println!("i is {}", *&i));
  thread::sleep(Duration::from_millis(10));
  println!("i : {:?}", i);

  let c = 'c';
  thread::spawn(move || println!("c is {}", &c));
  thread::sleep(Duration::from_millis(10));
  println!("c : {:?}", c);
}
