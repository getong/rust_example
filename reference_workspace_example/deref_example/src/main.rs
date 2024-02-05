use std::ops::Deref;

#[derive(Debug)]
struct DemoStruct {
  name: &'static str,
}

impl Deref for DemoStruct {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    println!("deref execute");
    &*self.name
  }
}

fn check(s: &str) {
  println!("check finish {}", s)
}

struct MyBox<T>(T);

impl<T> MyBox<T> {
  fn new(x: T) -> MyBox<T> {
    MyBox(x)
  }
}

impl<T> Deref for MyBox<T> {
  type Target = T;
  fn deref(&self) -> &T {
    &self.0
  }
}

fn main() {
  // println!("Hello, world!");
  let a = DemoStruct { name: "jack" };
  check(&a);

  let x = 5;
  let y = MyBox::new(x);
  assert_eq!(5, *y);
}
