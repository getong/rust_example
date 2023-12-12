/// Id of responses handler.
#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub struct HandlerId(usize, bool);

impl HandlerId {
  fn new(id: usize, respondable: bool) -> Self {
    HandlerId(id, respondable)
  }
  pub fn raw_id(self) -> usize {
    self.0
  }
  /// Indicates if a handler id corresponds to callback in the Agent runtime.
  pub fn is_respondable(self) -> bool {
    self.1
  }
}

fn main() {
  // println!("Hello, world!");
  let a: HandlerId = HandlerId::new(1, true);
  let b = a;
  println!("a: {:?}, {:p}\nb: {:?}, {:p}", a, &a, b, &b);
}
