use ark_ec::AdditiveGroup;
use ark_ff::Field;
use ark_std::{UniformRand, Zero, ops::Mul, test_rng};
// We'll use the BLS12-381 G1 curve for this example.
// This group has a prime order `r`, and is associated with a prime field `Fr`.
use ark_test_curves::bls12_381::{Fr as ScalarField, G1Projective as G};

fn main() {
  let mut rng = test_rng();
  // Let's sample uniformly random group elements:
  let a = G::rand(&mut rng);
  let b = G::rand(&mut rng);

  // We can add elements, ...
  let c = a + b;
  // ... subtract them, ...
  let d = a - b;
  // ... and double them.
  assert_eq!(c + d, a.double());
  // We can also negate elements, ...
  let e = -a;
  // ... and check that negation satisfies the basic group law
  assert_eq!(e + a, G::zero());

  // We can also multiply group elements by elements of the corresponding scalar field
  // (an act known as *scalar multiplication*)
  let scalar = ScalarField::rand(&mut rng);
  let e = c.mul(scalar);
  let f = e.mul(scalar.inverse().unwrap());
  assert_eq!(f, c);
}
