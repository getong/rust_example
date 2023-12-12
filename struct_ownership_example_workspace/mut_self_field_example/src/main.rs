struct Example {
  a: usize,
  b: usize,
}

impl Example {
  fn a(&mut self) -> &mut usize {
    &mut self.a
  }
  fn b(&mut self) -> &mut usize {
    &mut self.b
  }
}

fn main() {
  let mut s = Example { a: 1, b: 2 };

  // References created by returning &mut self.a and &mut self.b
  // from methods
  let _x = s.a();
  let _y = s.b();

  // References create by accessing the field directly
  let x = &mut s.a;
  let y = &mut s.b;

  // Ensure x/y aren't dropped due to non-lexical scope
  println!("x: {}", x);
  println!("y: {}", y);
}
