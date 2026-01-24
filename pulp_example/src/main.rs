use pulp::{Arch, Simd, WithSimd};

struct TimesThree<'a>(&'a mut [f64]);
impl<'a> WithSimd for TimesThree<'a> {
  type Output = ();

  #[inline(always)]
  fn with_simd<S: Simd>(self, simd: S) -> Self::Output {
    let v = self.0;
    let (head, tail) = S::as_mut_simd_f64s(v);

    let three = simd.splat_f64s(3.0);
    for x in head.iter_mut() {
      *x = simd.mul_f64s(three, *x);
    }

    for x in tail.iter_mut() {
      *x = *x * 3.0;
    }
  }
}

fn main() {
  // println!("Hello, world!");
  let mut v = (0 .. 1000).map(|i| i as f64).collect::<Vec<_>>();
  let arch = Arch::new();

  arch.dispatch(|| {
    for x in &mut v {
      *x *= 2.0;
    }
  });

  for (i, x) in v.into_iter().enumerate() {
    assert_eq!(x, 2.0 * i as f64);
  }

  let mut v = (0 .. 1000).map(|i| i as f64).collect::<Vec<_>>();
  let arch = Arch::new();

  arch.dispatch(TimesThree(&mut v));

  for (i, x) in v.into_iter().enumerate() {
    assert_eq!(x, 3.0 * i as f64);
  }
}
