use symbolica::{
  atom::{Atom, AtomCore},
  domains::{integer::Z, rational::Q, rational_polynomial::RationalPolynomialField},
  parse,
  tensors::matrix::Matrix,
};

fn main() {
  let t = parse!("t");
  let data = [t.clone(), 2.into(), 3.into(), t + 1];
  let rhs: [Atom; 2] = [3.into(), 4.into()];
  let res = Matrix::from_linear(
    data
      .into_iter()
      .map(|x| x.to_rational_polynomial(&Q, &Z, None))
      .collect(),
    2,
    2,
    RationalPolynomialField::<_, u8>::new(Z),
  )
  .unwrap();
  let rhs = Matrix::new_vec(
    rhs
      .into_iter()
      .map(|x| x.to_rational_polynomial(&Q, &Z, None))
      .collect(),
    RationalPolynomialField::new(Z),
  );

  let res = res.solve(&rhs).unwrap();
  println!("{}", res);
}
