use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_std::{UniformRand, test_rng};
use ark_test_curves::bls12_381::{G1Affine, G1Projective as G1, G2Affine, G2Projective as G2};

fn main() {
  // We'll use the BLS12-381 pairing-friendly group for this example.

  let mut rng = test_rng();
  // Let's sample uniformly random group elements:
  let a: G1Affine = G1::rand(&mut rng).into();
  let _b: G2Affine = G2::rand(&mut rng).into();

  // We can serialize with compression...
  let mut compressed_bytes = Vec::new();
  a.serialize_compressed(&mut compressed_bytes).unwrap();
  // ...and without:
  let mut uncompressed_bytes = Vec::new();
  a.serialize_uncompressed(&mut uncompressed_bytes).unwrap();

  // We can reconstruct our points from the compressed serialization...
  let a_compressed = G1Affine::deserialize_compressed(&*compressed_bytes).unwrap();

  // ... and from the uncompressed one:
  let a_uncompressed = G1Affine::deserialize_uncompressed(&*uncompressed_bytes).unwrap();

  assert_eq!(a_compressed, a);
  assert_eq!(a_uncompressed, a);

  // If we trust the origin of the serialization
  // (eg: if the serialization was stored on authenticated storage),
  // then we can skip some validation checks, which can greatly reduce deserialization time.
  let a_uncompressed_unchecked =
    G1Affine::deserialize_uncompressed_unchecked(&*uncompressed_bytes).unwrap();
  let a_compressed_unchecked =
    G1Affine::deserialize_compressed_unchecked(&*compressed_bytes).unwrap();
  assert_eq!(a_uncompressed_unchecked, a);
  assert_eq!(a_compressed_unchecked, a);
}
