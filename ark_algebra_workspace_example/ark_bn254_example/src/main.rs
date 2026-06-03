use ark_bn254::{Bn254, Fq, Fq2, Fr, G1Projective, G2Projective};
use ark_ec::{CurveGroup, PrimeGroup, ScalarMul, VariableBaseMSM, pairing::Pairing};
use ark_ff::{Field, One, Zero};

// ─── 1. Arithmetic Circuit ────────────────────────────────────────────────────
// 在 zkSNARK 中，prover 拥有 *私有的 witness* x，想要说服 verifier
// 公开输出 y 满足 y = f(x)。这里 f(x) = x² + 3x + 5。
// 约束系统在 BN254 的标量域 Fr 中检查。

fn cubic_poly(witness: Fr) -> Fr {
  witness.square() + Fr::from(3u64) * witness + Fr::from(5u64)
}

fn verify_cubic(witness: Fr, claimed_output: Fr) -> bool {
  cubic_poly(witness) == claimed_output
}

// ─── 2. Pedersen Commitment ───────────────────────────────────────────────────
// Pedersen 承诺 C = v·G + r·H 将值 v 绑定到一个点 C。
// 盲化因子 r 对 verifier 隐藏 v；committer 之后可以通过透露 (v, r) *打开* 承诺。
// 绑定性成立是因为找到不同的 (v', r') 来打开 C 需要求解离散对数。

struct PedersenCommitment {
  value: Fr,
  blinding: Fr,
  commitment: G1Projective,
}

/// 生成独立的"第二生成元"H，其相对于 G 的离散对数是未知的。
/// 生产环境中 H 应通过 nothing-up-my-sleeve (NUMS) hash-to-curve 导出。
fn pedersen_generator_h() -> G1Projective {
  G1Projective::generator() * Fr::from(13u64)
}

fn pedersen_commit(value: Fr, blinding: Fr) -> PedersenCommitment {
  let g = G1Projective::generator();
  let h = pedersen_generator_h();
  let commitment = g * value + h * blinding;
  PedersenCommitment {
    value,
    blinding,
    commitment,
  }
}

fn pedersen_open(c: &PedersenCommitment) -> bool {
  let g = G1Projective::generator();
  let h = pedersen_generator_h();
  c.commitment == g * c.value + h * c.blinding
}

// ─── 3. Aggregated Pedersen (Multi-Value Commitment) ─────────────────────────
// 用 k+1 个独立生成元承诺多个值 (v₁,…,vₖ) 到一个群元素：
// C = Σ vᵢ·Gᵢ + r·H。这是机密交易（Confidential Transactions）的核心构建块。

struct AggregatedCommitment {
  values: Vec<Fr>,
  blinding: Fr,
  commitment: G1Projective,
}

/// 通过连续的标量乘法派生一批独立生成元（仅示例；生产代码应使用 hash-to-curve）。
fn aggregated_generators(n: usize) -> Vec<G1Projective> {
  let g = G1Projective::generator();
  (0 .. n).map(|i| g * Fr::from(13u64 + i as u64)).collect()
}

fn aggregated_commit(values: Vec<Fr>, blinding: Fr) -> AggregatedCommitment {
  let gens = aggregated_generators(values.len());
  let h = pedersen_generator_h();
  let mut all_points: Vec<G1Projective> = gens;
  all_points.push(h);
  let all_affines = G1Projective::batch_convert_to_mul_base(&all_points);
  let scalars: Vec<Fr> = values
    .iter()
    .copied()
    .chain(std::iter::once(blinding))
    .collect();
  let commitment = G1Projective::msm(&all_affines, &scalars).unwrap();
  AggregatedCommitment {
    values,
    blinding,
    commitment,
  }
}

fn aggregated_open(c: &AggregatedCommitment) -> bool {
  let mut all_points = aggregated_generators(c.values.len());
  all_points.push(pedersen_generator_h());
  let all_affines = G1Projective::batch_convert_to_mul_base(&all_points);
  let scalars: Vec<Fr> = c
    .values
    .iter()
    .copied()
    .chain(std::iter::once(c.blinding))
    .collect();
  let recomputed = G1Projective::msm(&all_affines, &scalars).unwrap();
  c.commitment == recomputed
}

// ─── 4. Sigma Protocol — Proof of Knowledge of Discrete Log ──────────────────
// Prover 知道 x 使得 P = x·G。协议：
//   Prover: 选随机数 r ← Fr，发送 R = r·G
//   Verifier: 发送挑战 c ← Fr
//   Prover: 响应 s = r + c·x
//   Verifier: 检查 s·G = R + c·P
// 这里使用 Fiat-Shamir 变换使其非交互。

struct DLogProof {
  challenge: Fr,
  response: Fr,
}

fn prove_dlog(secret: Fr) -> DLogProof {
  let g = G1Projective::generator();
  let _public_key = g * secret;

  // Prover: pick random nonce r, compute R = r·G
  let r = Fr::from(17u64); // In reality, sample securely
  let r_g = g * r;

  // Fiat-Shamir: challenge = Hash(G || P || R)
  let challenge = Fr::from(99u64); // In reality, hash to Fr

  // Response: s = r + challenge * secret
  let response = r + challenge * secret;

  // Self-check: s·G == R + challenge·P
  let lhs = g * response;
  let rhs = r_g + _public_key * challenge;
  assert_eq!(lhs, rhs, "Sigma protocol self-check failed");

  DLogProof {
    challenge,
    response,
  }
}

fn verify_dlog(public_key: G1Projective, proof: &DLogProof) -> bool {
  let g = G1Projective::generator();
  // Recompute R from response: R = s·G - c·P
  let r_g = g * proof.response - public_key * proof.challenge;
  // Recompute challenge (in real code: hash(G || P || R))
  let expected_challenge = Fr::from(99u64);
  expected_challenge == proof.challenge && !r_g.is_zero()
}

// ─── 5. BLS Signature Verification (Pairing-based) ────────────────────────────
// BLS 使用配对 e: G1 × G2 → GT。
//   KeyGen:  sk = s,  pk = s·G₂
//   Sign:    σ = s·H(m)  其中 H(m) ∈ G₁
//   Verify:  e(σ, G₂) = e(H(m), pk)
// 这里演示的是"最小公钥大小"变体：pk ∈ G₂, σ ∈ G₁。

fn bls_sign(secret_key: Fr) -> G1Projective {
  let hash_to_g1 = G1Projective::generator() * Fr::from(42u64); // demo hash
  hash_to_g1 * secret_key
}

fn bls_verify(signature: G1Projective, public_key: G2Projective) -> bool {
  let hash_to_g1 = G1Projective::generator() * Fr::from(42u64); // demo hash
  let lhs = Bn254::pairing(
    signature.into_affine(),
    G2Projective::generator().into_affine(),
  );
  let rhs = Bn254::pairing(hash_to_g1.into_affine(), public_key.into_affine());
  lhs == rhs
}

// ─── 6. Inner-Product Argument (scalar-product relation) ──────────────────────
// 证明 c = Σ aᵢ·bᵢ 而不泄露 a 或 b。
// 这是 Bulletproofs 和 Range Proofs 的核心。这里只是公开验证关系；
// 真实的 IPA 会使用递归二分协议。

fn inner_product(a: &[Fr], b: &[Fr]) -> Fr {
  a.iter()
    .zip(b.iter())
    .fold(Fr::zero(), |acc, (x, y)| acc + *x * *y)
}

fn verify_inner_product(a: &[Fr], b: &[Fr], claimed: Fr) -> bool {
  inner_product(a, b) == claimed
}

// ─── 7. GT Target Group Arithmetic ────────────────────────────────────────────
// 配对输出 GT 中的元素。一些证明系统（如 Groth16 验证器、BLS）
// 需要在 GT 中做乘法和指数运算。GT 是乘法群，但 ark-ec 用加法记号：
//   PairingOutput + PairingOutput = 群运算（乘法意义下的 GT 乘法）
//   PairingOutput * ScalarField  = 标量乘法（乘法意义下的 GT 指数）

fn gt_operations() {
  let g1 = G1Projective::generator().into_affine();
  let g2 = G2Projective::generator().into_affine();
  let base = Bn254::pairing(g1, g2);

  // GT 中 pairings 输出相乘（对应加法记号）：base^2 = base + base
  let squared = base + base;
  // base^3 = base^2 + base
  let cubed = squared + base;

  // 标量乘法即指数：base^k = base * k
  let k = Fr::from(3u64);
  let exp = base * k;

  assert!(!base.is_zero(), "e(G1, G2) is non-identity in GT");
  assert_eq!(cubed, exp, "GT: base^3 via repeated add == base * 3");
  println!("   GT: base = e(G₁, G₂)");
  println!("   base^3 = e(G₁, G₂)³ ✓");
}

// ─── 8. Field Operations Showcase ─────────────────────────────────────────────
// BN254 的标量域 Fr 和基域 Fq 支持大量代数运算，在 ZK 电路内部被广泛使用。

fn field_operations() {
  let a = Fr::from(7u64);
  let b = Fr::from(11u64);

  let sum = a + b;
  let diff = a - b;
  let prod = a * b;

  assert_eq!(sum, Fr::from(18u64));
  assert_eq!(diff, Fr::from(-4i64)); // -4 mod q
  assert_eq!(prod, Fr::from(77u64));

  // Legendre symbol / quadratic residuosity
  let _is_qr = a.legendre().is_qr();

  // Square root (if exists)
  let four = Fr::from(4u64);
  let sqrt_4 = four.sqrt().unwrap();
  assert!(sqrt_4 == Fr::from(2u64) || sqrt_4 == -Fr::from(2u64));

  // Inverse
  let inv = a.inverse().unwrap();
  assert_eq!(a * inv, Fr::one());

  // Frobenius (only meaningful in extension fields)
  let _fq2_frobenius = Fq2::new(Fq::from(1u64), Fq::from(2u64)).frobenius_map(1);

  println!("   a = {a:?}, b = {b:?}");
  println!("   a + b = {sum:?}");
  println!("   a * b = {prod:?}");
  println!("   sqrt(4) = {sqrt_4:?}");
  println!("   a * a⁻¹ = 1 ✓");
}

// ─── 9. Bilinearity Check ────────────────────────────────────────────────────
// 配对双线性：e(a·G₁, b·G₂) = e(G₁, G₂)^{a·b}
// 这是所有基于配对的密码学（BLS、Groth16、Plonk）的数学基础。

fn check_bilinearity() {
  let g1 = G1Projective::generator().into_affine();
  let g2 = G2Projective::generator().into_affine();
  let a = Fr::from(3u64);
  let b = Fr::from(7u64);

  let left = Bn254::pairing((g1 * a).into_affine(), (g2 * b).into_affine());
  let right = Bn254::pairing(g1, g2) * (a * b);

  assert_eq!(left, right, "pairing bilinearity holds");
  println!("   e(a*G1, b*G2) = e(G1, G2)^(a*b)  ✓");
}

// ─── Main ─────────────────────────────────────────────────────────────────────

fn main() {
  println!("═══ BN254 ZKP Primitives Showcase ═══\n");

  // 1. Arithmetic circuit
  let witness = Fr::from(7u64);
  let output = cubic_poly(witness);
  assert!(verify_cubic(witness, output));
  println!("1. Arithmetic circuit: y = x² + 3x + 5");
  println!("   x = {witness:?}  →  y = {output:?}\n");

  // 2. Pedersen commitment
  let com = pedersen_commit(Fr::from(42u64), Fr::from(19u64));
  assert!(pedersen_open(&com));
  println!("2. Pedersen commitment: C = v·G + r·H");
  println!("   C = {:?}\n", com.commitment);

  // 3. Aggregated (multi-value) Pedersen commitment
  let values = vec![Fr::from(1u64), Fr::from(2u64), Fr::from(3u64)];
  let agg = aggregated_commit(values, Fr::from(99u64));
  assert!(aggregated_open(&agg));
  println!("3. Aggregated Pedersen: C = Σ vᵢ·Gᵢ + r·H (uses MSM)");
  println!("   values = [1, 2, 3], C = {:?}\n", agg.commitment);

  // 4. Sigma protocol (non-interactive proof of dlog knowledge)
  let secret = Fr::from(123u64);
  let proof = prove_dlog(secret);
  let pk = G1Projective::generator() * secret;
  assert!(verify_dlog(pk, &proof));
  println!("4. Sigma protocol (Fiat-Shamir) — proof of dlog knowledge ✓\n");

  // 5. BLS signature
  let sk = Fr::from(99u64);
  let pk_g2 = G2Projective::generator() * sk;
  let sig = bls_sign(sk);
  assert!(bls_verify(sig, pk_g2));
  println!("5. BLS signature: e(σ, G₂) = e(H(m), pk) ✓\n");

  // 6. Inner product
  let a = vec![Fr::from(1u64), Fr::from(2u64), Fr::from(3u64)];
  let b = vec![Fr::from(4u64), Fr::from(5u64), Fr::from(6u64)];
  let c = inner_product(&a, &b); // 1*4 + 2*5 + 3*6 = 32
  assert!(verify_inner_product(&a, &b, c));
  println!("6. Inner product: ⟨a, b⟩ = {c:?} (core of Bulletproofs)\n");

  // 7. GT target group
  println!("7. GT target group:");
  gt_operations();
  println!();

  // 8. Field operations
  println!("8. Fr/Fq field operations:");
  field_operations();
  println!();

  // 9. Pairing bilinearity
  println!("9. Pairing bilinearity:");
  check_bilinearity();
  println!();

  // Summary
  println!("── Summary ──");
  println!("BN254 (alt_bn128) is a pairing-friendly elliptic curve");
  println!("  • Scalar field Fr :: 254 bits");
  println!("  • Base field  Fq :: 254 bits");
  println!("  • Embedding degree :: 12");
  println!("  • Uses: Groth16, Plonk, EVM precompiles (0x06, 0x07, 0x08)");
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_cubic_poly_correct() {
    let x = Fr::from(7u64);
    let y = cubic_poly(x);
    assert!(verify_cubic(x, y));
  }

  #[test]
  fn test_cubic_poly_wrong_output_rejected() {
    let x = Fr::from(7u64);
    let mut y = cubic_poly(x);
    y += Fr::one();
    assert!(!verify_cubic(x, y));
  }

  #[test]
  fn test_cubic_poly_zero() {
    assert_eq!(cubic_poly(Fr::zero()), Fr::from(5u64));
  }

  #[test]
  fn test_pedersen_commitment_honest() {
    let c = pedersen_commit(Fr::from(42u64), Fr::from(19u64));
    assert!(pedersen_open(&c));
  }

  #[test]
  fn test_pedersen_commitment_wrong_value_rejected() {
    let c = pedersen_commit(Fr::from(42u64), Fr::from(19u64));
    let g = G1Projective::generator();
    let h = pedersen_generator_h();
    assert!(g * Fr::from(99u64) + h * c.blinding != c.commitment);
  }

  #[test]
  fn test_aggregated_commitment() {
    let values = vec![Fr::from(10u64), Fr::from(20u64), Fr::from(30u64)];
    let c = aggregated_commit(values, Fr::from(77u64));
    assert!(aggregated_open(&c));
  }

  #[test]
  fn test_aggregated_commitment_empty() {
    let values = vec![];
    let c = aggregated_commit(values, Fr::from(1u64));
    assert!(aggregated_open(&c));
  }

  #[test]
  fn test_sigma_protocol() {
    let secret = Fr::from(42u64);
    let proof = prove_dlog(secret);
    let pk = G1Projective::generator() * secret;
    assert!(verify_dlog(pk, &proof));
  }

  #[test]
  fn test_sigma_protocol_tampered_proof_rejected() {
    let secret = Fr::from(42u64);
    let mut proof = prove_dlog(secret);
    proof.challenge = Fr::from(0u64); // tamper with the challenge
    let pk = G1Projective::generator() * secret;
    assert!(!verify_dlog(pk, &proof));
  }

  #[test]
  fn test_bls_honest() {
    let sk = Fr::from(42u64);
    let pk = G2Projective::generator() * sk;
    let sig = bls_sign(sk);
    assert!(bls_verify(sig, pk));
  }

  #[test]
  fn test_bls_tampered_key_rejected() {
    let sk = Fr::from(42u64);
    let sig = bls_sign(sk);
    let wrong_pk = G2Projective::generator() * Fr::from(999u64);
    assert!(!bls_verify(sig, wrong_pk));
  }

  #[test]
  fn test_inner_product_basic() {
    let a = vec![Fr::from(1u64), Fr::from(2u64)];
    let b = vec![Fr::from(3u64), Fr::from(4u64)];
    assert_eq!(inner_product(&a, &b), Fr::from(11u64));
  }

  #[test]
  fn test_inner_product_empty() {
    let a: Vec<Fr> = vec![];
    let b: Vec<Fr> = vec![];
    assert_eq!(inner_product(&a, &b), Fr::zero());
  }

  #[test]
  fn test_gt_arithmetic() {
    gt_operations();
  }

  #[test]
  fn test_pairing_bilinearity() {
    check_bilinearity();
  }

  #[test]
  fn test_pairing_non_degeneracy() {
    let g1 = G1Projective::generator().into_affine();
    let g2 = G2Projective::generator().into_affine();
    let e = Bn254::pairing(g1, g2);
    assert!(!e.is_zero(), "e(G1, G2) ≠ 0");
  }

  #[test]
  fn test_field_inverse() {
    let a = Fr::from(5u64);
    let inv = a.inverse().unwrap();
    assert_eq!(a * inv, Fr::one());
  }

  #[test]
  fn test_field_sqrt() {
    let four = Fr::from(4u64);
    let sqrt = four.sqrt();
    assert!(sqrt.is_some());
    assert_eq!(sqrt.unwrap().square(), four);
  }

  #[test]
  fn test_msm_consistency() {
    let gens: Vec<G1Projective> = (0 .. 3).map(|_| G1Projective::generator()).collect();
    let scalars = vec![Fr::from(1u64), Fr::from(2u64), Fr::from(3u64)];
    let affines = G1Projective::batch_convert_to_mul_base(&gens);
    let msm_result = G1Projective::msm(&affines, &scalars).unwrap();
    let expected = G1Projective::generator() * (Fr::from(1u64) + Fr::from(2u64) + Fr::from(3u64));
    assert_eq!(msm_result, expected);
  }

  #[test]
  fn test_g1_order() {
    let r = Fr::characteristic();
    let g1 = G1Projective::generator();
    assert_eq!(g1.mul_bigint(r), G1Projective::zero());
  }

  #[test]
  fn test_g2_order() {
    let r = Fr::characteristic();
    let g2 = G2Projective::generator();
    assert_eq!(g2.mul_bigint(r), G2Projective::zero());
  }
}
