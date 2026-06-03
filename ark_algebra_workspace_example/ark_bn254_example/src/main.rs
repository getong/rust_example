use ark_bn254::{Bn254, Fr, G1Projective, G2Projective};
use ark_ec::{CurveGroup, PrimeGroup, pairing::Pairing};
use ark_ff::{Field, Zero};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CircuitClaim {
  witness: Fr,
  public_output: Fr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PedersenOpening {
  value: Fr,
  blinding: Fr,
  commitment: G1Projective,
}

fn main() {
  let claim = arithmetic_circuit_claim(Fr::from(7_u64));
  assert!(verify_arithmetic_relation(claim));
  println!("1. Arithmetic circuit: checked y = x^2 + 3x + 5 over the BN254 scalar field.");

  let opening = pedersen_commitment(Fr::from(42_u64), Fr::from(19_u64));
  assert!(verify_pedersen_opening(opening));
  println!("2. Commitment: bound a private value to a G1 point with a blinding factor.");

  assert!(verify_pairing_relation(Fr::from(11_u64)));
  println!("3. Pairing check: verified e(xG1, G2) = e(G1, xG2), the shape used by Groth16.");

  println!("BN254 is a pairing-friendly curve used by many zkSNARK systems and EVM precompiles.");
}

fn arithmetic_circuit_claim(witness: Fr) -> CircuitClaim {
  let public_output = witness.square() + Fr::from(3_u64) * witness + Fr::from(5_u64);
  CircuitClaim {
    witness,
    public_output,
  }
}

fn verify_arithmetic_relation(claim: CircuitClaim) -> bool {
  let recomputed = claim.witness.square() + Fr::from(3_u64) * claim.witness + Fr::from(5_u64);
  recomputed == claim.public_output
}

fn pedersen_commitment(value: Fr, blinding: Fr) -> PedersenOpening {
  assert!(!blinding.is_zero());

  let g = G1Projective::generator();
  let h = secondary_demo_generator();
  let commitment = g * value + h * blinding;

  PedersenOpening {
    value,
    blinding,
    commitment,
  }
}

fn verify_pedersen_opening(opening: PedersenOpening) -> bool {
  let g = G1Projective::generator();
  let h = secondary_demo_generator();
  opening.commitment == g * opening.value + h * opening.blinding
}

fn verify_pairing_relation(secret: Fr) -> bool {
  let g1 = G1Projective::generator();
  let g2 = G2Projective::generator();

  let proof_point = (g1 * secret).into_affine();
  let public_key = (g2 * secret).into_affine();

  let left = Bn254::pairing(proof_point, g2.into_affine());
  let right = Bn254::pairing(g1.into_affine(), public_key);

  left == right
}

fn secondary_demo_generator() -> G1Projective {
  // Demo only: production Pedersen commitments need an independent generator
  // whose discrete-log relation to G1 is unknown.
  G1Projective::generator() * Fr::from(13_u64)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn checks_arithmetic_claim() {
    let claim = arithmetic_circuit_claim(Fr::from(7_u64));
    assert!(verify_arithmetic_relation(claim));
  }

  #[test]
  fn rejects_wrong_public_output() {
    let mut claim = arithmetic_circuit_claim(Fr::from(7_u64));
    claim.public_output += Fr::from(1_u64);
    assert!(!verify_arithmetic_relation(claim));
  }

  #[test]
  fn verifies_commitment_opening() {
    let opening = pedersen_commitment(Fr::from(42_u64), Fr::from(19_u64));
    assert!(verify_pedersen_opening(opening));
  }

  #[test]
  fn verifies_pairing_relation() {
    assert!(verify_pairing_relation(Fr::from(11_u64)));
  }
}
