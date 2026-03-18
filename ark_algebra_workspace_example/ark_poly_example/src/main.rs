use ark_poly::{
  DenseMVPolynomial, Polynomial,
  polynomial::multivariate::{SparsePolynomial, SparseTerm, Term},
};
use ark_test_curves::bls12_381::Fq;

fn main() {
  // Create a multivariate polynomial in 3 variables, with 4 terms:
  // f(x_0, x_1, x_2) = 2*x_0^3 + x_0*x_2 + x_1*x_2 + 5
  let poly = SparsePolynomial::from_coefficients_vec(
    3,
    vec![
      (Fq::from(2), SparseTerm::new(vec![(0, 3)])),
      (Fq::from(1), SparseTerm::new(vec![(0, 1), (2, 1)])),
      (Fq::from(1), SparseTerm::new(vec![(1, 1), (2, 1)])),
      (Fq::from(5), SparseTerm::new(vec![])),
    ],
  );
  assert_eq!(
    poly.evaluate(&vec![Fq::from(2), Fq::from(3), Fq::from(6)]),
    Fq::from(51)
  );
}
