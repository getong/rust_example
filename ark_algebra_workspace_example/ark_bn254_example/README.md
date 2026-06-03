# ark_bn254 ZKP Example

This crate demonstrates the core primitives exposed by `ark-bn254` and how they map to zero-knowledge proof systems.

## What The Code Shows

1. `Fr` arithmetic models arithmetic-circuit constraints.
   The example checks `y = x^2 + 3x + 5` in the BN254 scalar field. In a real ZKP, `x` is private witness data and `y` can be public input.

2. `G1Projective` scalar multiplication models commitments.
   The example builds a Pedersen-style commitment `C = value * G + blinding * H`. Commitments are used to bind a private value while hiding it.

3. `Bn254::pairing` models verifier equations.
   The example checks `e(xG1, G2) = e(G1, xG2)`. zkSNARK verifiers such as Groth16 use BN254 pairings to verify compact proofs quickly.

## ZKP Applications

- Private payments and asset transfers: prove balance and authorization without revealing transaction details.
- Identity and credentials: prove age, membership, or authorization without exposing the underlying document.
- Rollups and blockchain scaling: prove many off-chain state transitions with one short on-chain verification.
- Verifiable computation: prove a computation was executed correctly without rerunning the whole computation.

## Notes

This is a teaching example, not a complete proof system. Production ZKP code normally uses libraries such as `ark-groth16`, `ark-relations`, or a PLONK/STARK framework, and must use trusted setup or transparent setup rules appropriate to the protocol.
