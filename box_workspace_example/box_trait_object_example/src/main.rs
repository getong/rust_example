use std::fmt::Debug;

trait Object: Debug {}

#[derive(Debug)]
struct Holder<'lifetime> {
  objects: Vec<Box<dyn Object + 'lifetime>>,
}

impl<'lifetime> Holder<'lifetime> {
  fn add_object<T: Object + 'lifetime>(self: &'_ mut Self, object: T) {
    self.objects.push(Box::new(object));
  }
}

#[derive(Debug)]
pub struct A {
  pub a: u8,
}

impl Object for A {}

fn main() {
  // println!("Hello, world!");
  let mut holder = Holder {
    objects: Vec::new(),
  };
  holder.add_object(A { a: 1u8 });
  println!("holder:{:?}", holder);
}
