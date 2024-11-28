use std::{
  convert::From,
  fmt::{Display, Formatter, Result},
  ops::Add,
};

#[derive(Default, Debug, PartialEq, Copy, Clone)]
struct Complex<T> {
  re: T,
  im: T,
}

impl<T> Complex<T> {
  fn new(re: T, im: T) -> Self {
    Complex { re, im }
  }
}

impl<T: Add<T, Output = T>> Add for Complex<T> {
  type Output = Complex<T>;
  fn add(self, rhs: Complex<T>) -> Self::Output {
    Complex {
      re: self.re + rhs.re,
      im: self.im + rhs.im,
    }
  }
}

impl<T> From<(T, T)> for Complex<T> {
  fn from(value: (T, T)) -> Complex<T> {
    Complex {
      re: value.0,
      im: value.1,
    }
  }
}

impl<T: Display> Display for Complex<T> {
  fn fmt(&self, f: &mut Formatter) -> Result {
    write!(f, "{} + {}i", self.re, self.im)
  }
}

fn main() {
  println!("Hello, world!");
}

#[cfg(test)]
mod tests {
  use crate::Complex;

  #[test]
  fn complex_basics() {
    let _first = Complex::new(3, 5);
    let _second: Complex<i32> = Complex::default();
  }

  fn complex_addition() {
    let a = Complex::new(1, -2);
    let b = Complex::default();
    let res = a + b;
    assert_eq!(res, a);
  }

  #[test]
  fn complex_from() {
    let a = (2345, 456);
    let complex = Complex::from(a);
    assert_eq!(complex.re, 2345);
    assert_eq!(complex.im, 456);
  }

  #[test]
  fn complex_display() {
    let my_imaginary = Complex::new(2345, 456);
    println!("{}", my_imaginary);
  }
}
