use validit::Valid;

struct Foo(u64);

impl validit::Validate for Foo {
  fn validate(&self) -> Result<(), Box<dyn std::error::Error>> {
    validit::less!(self.0, 5);
    Ok(())
  }
}

fn main() {
  let v1 = Valid::new(Foo(1));
  let _x = v1.0; // Good.

  let v6 = Foo(6);
  let _x = v6.0; // No panic without validation.

  let v6 = Valid::new(Foo(6));
  let _x = v6.0; // panic: panicked at 'invalid state: expect: self.0(6) < 5(5) ...
}
