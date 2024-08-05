use winterfell::math::{fields::f128::BaseElement, FieldElement};

fn do_work(start: BaseElement, n: usize) -> BaseElement {
  let mut result = start;
  for _ in 1 .. n {
    result = result.exp(3) + BaseElement::new(42);
  }
  result
}

fn main() {
  // println!("Hello, world!");
}
