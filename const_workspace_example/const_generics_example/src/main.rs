use std::{
  fmt::Debug,
  ops::{Add, Mul},
};

#[derive(Copy, Clone, Debug)]
struct Matrix<T: Copy + Debug, const N: usize, const M: usize>([[T; M]; N]);

impl<T: Copy + Debug, const N: usize, const M: usize> Matrix<T, N, M> {
  pub fn new(v: [[T; M]; N]) -> Self {
    Self(v)
  }

  pub fn with_all(v: T) -> Self {
    Self([[v; M]; N])
  }
}

impl<T: Copy + Default + Debug, const N: usize, const M: usize> Default for Matrix<T, N, M> {
  fn default() -> Self {
    Self::with_all(Default::default())
  }
}

impl<T, const N: usize, const M: usize, const L: usize> Mul<Matrix<T, M, L>> for Matrix<T, N, M>
where
  T: Copy + Default + Add<T, Output = T> + Mul<T, Output = T> + Debug,
{
  type Output = Matrix<T, N, L>;

  fn mul(self, rhs: Matrix<T, M, L>) -> Self::Output {
    let mut out: Self::Output = Default::default();

    for r in 0..N {
      for c in 0..M {
        for l in 0..L {
          out.0[r][l] = out.0[r][l] + self.0[r][c] * rhs.0[c][l];
        }
      }
    }

    out
  }
}

type Vector<T, const N: usize> = Matrix<T, N, 1usize>;

fn main() {
  let m = Matrix::new([[1f64, 0f64, 0f64], [1f64, 2f64, 0f64], [1f64, 2f64, 3f64]]);
  let v = Vector::new([[10f64], [20f64], [40f64]]);

  println!("{:?} * {:?} = {:?}", m, v, m * v);
}
