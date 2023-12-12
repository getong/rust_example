// chapter2/extending-types.rs
// Trait for our behavior
trait Sawtooth {
  fn sawtooth(&self) -> Self;
}

// Extending the builtin f64 type
impl Sawtooth for f64 {
  fn sawtooth(&self) -> f64 {
    self - self.floor()
  }
}

fn main() {
  println!("{}", 2.34f64.sawtooth());
}
