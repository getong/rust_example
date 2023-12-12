use std::fmt::Debug;

trait Animal: Debug {
  fn walk(&self) {
    println!("walk");
  }
}

impl dyn Animal {
  fn talk() {
    println!("talk");
  }
}

#[derive(Debug)]
struct Person;

impl Animal for Person {}

fn demo() -> Box<dyn Animal> {
  let p = Person;
  Box::new(p)
}

fn main() {
  // println!("Hello, world!");
  let p = Person;

  p.walk();

  let p1 = demo();
  p1.walk();
  p1.talk();
}
