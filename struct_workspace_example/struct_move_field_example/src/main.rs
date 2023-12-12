#[derive(Debug)]
pub struct Foo {}

impl Foo {
  pub fn new() -> Foo {
    Foo {}
  }
}

#[derive(Debug)]
pub struct Bar {
  foo: Foo,
}

impl Bar {
  pub fn swap(&mut self) -> Foo {
    // let ans = self.foo;
    // self.foo = Foo::new();
    // ans
    std::mem::replace(&mut self.foo, Foo::new())
  }
}

fn main() {
  println!("Hello, world!");
  let foo = Foo::new();

  println!("foo:{:#?}", foo);
  let mut bar = Bar { foo: foo };
  bar.swap();
  println!("bar:{:#?}", bar);
}
