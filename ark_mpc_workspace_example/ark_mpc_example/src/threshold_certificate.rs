//! Small certificate-shaped examples for share reconstruction.
//!
//! Renegade's `SecretShareType` model is additive: one private share plus one
//! public share reconstructs the base value by field-wise addition. This module
//! includes that pattern and a separate Shamir-style threshold example where
//! any `threshold` shares out of `total` can reconstruct the same certificate.

use std::collections::HashSet;

use crate::MpcScalar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MiniCertificate {
  pub serial: MpcScalar,
  pub holder_id: MpcScalar,
  pub issuer_id: MpcScalar,
}

impl MiniCertificate {
  pub fn new(serial: u64, holder_id: u64, issuer_id: u64) -> Self {
    Self {
      serial: MpcScalar::from(serial),
      holder_id: MpcScalar::from(holder_id),
      issuer_id: MpcScalar::from(issuer_id),
    }
  }

  fn to_scalars(self) -> [MpcScalar; 3] {
    [self.serial, self.holder_id, self.issuer_id]
  }

  fn from_scalars(scalars: [MpcScalar; 3]) -> Self {
    Self {
      serial: scalars[0],
      holder_id: scalars[1],
      issuer_id: scalars[2],
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdditiveCertificateShare {
  pub serial: MpcScalar,
  pub holder_id: MpcScalar,
  pub issuer_id: MpcScalar,
}

impl AdditiveCertificateShare {
  fn from_scalars(scalars: [MpcScalar; 3]) -> Self {
    Self {
      serial: scalars[0],
      holder_id: scalars[1],
      issuer_id: scalars[2],
    }
  }

  fn to_scalars(self) -> [MpcScalar; 3] {
    [self.serial, self.holder_id, self.issuer_id]
  }

  pub fn add_shares(self, rhs: Self) -> MiniCertificate {
    let lhs = self.to_scalars();
    let rhs = rhs.to_scalars();

    MiniCertificate::from_scalars([lhs[0] + rhs[0], lhs[1] + rhs[1], lhs[2] + rhs[2]])
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThresholdCertificateShare {
  pub index: u64,
  pub serial: MpcScalar,
  pub holder_id: MpcScalar,
  pub issuer_id: MpcScalar,
}

impl ThresholdCertificateShare {
  fn from_scalars(index: u64, scalars: [MpcScalar; 3]) -> Self {
    Self {
      index,
      serial: scalars[0],
      holder_id: scalars[1],
      issuer_id: scalars[2],
    }
  }

  fn scalar_at_field(self, field_idx: usize) -> MpcScalar {
    match field_idx {
      0 => self.serial,
      1 => self.holder_id,
      2 => self.issuer_id,
      _ => panic!("invalid certificate field index"),
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThresholdCertificateExample {
  pub threshold: usize,
  pub total_shares: usize,
  pub used_indices: Vec<u64>,
  pub recovered: MiniCertificate,
}

pub fn split_certificate_additively(
  certificate: MiniCertificate,
  private_share: AdditiveCertificateShare,
) -> (AdditiveCertificateShare, AdditiveCertificateShare) {
  let certificate = certificate.to_scalars();
  let private = private_share.to_scalars();
  let public = [
    certificate[0] - private[0],
    certificate[1] - private[1],
    certificate[2] - private[2],
  ];

  (
    private_share,
    AdditiveCertificateShare::from_scalars(public),
  )
}

pub fn split_certificate_threshold(
  certificate: MiniCertificate,
  total: usize,
  threshold: usize,
) -> Result<Vec<ThresholdCertificateShare>, String> {
  if threshold < 2 {
    return Err("threshold must be at least 2".to_string());
  }
  if threshold > total {
    return Err("threshold must be less than or equal to total".to_string());
  }

  let secrets = certificate.to_scalars();
  let mut shares = Vec::with_capacity(total);

  for index in 1 ..= total as u64 {
    let x = MpcScalar::from(index);
    let mut scalars = [MpcScalar::from(0u64); 3];

    for field_idx in 0 .. 3 {
      let coefficients = deterministic_polynomial(secrets[field_idx], field_idx, threshold);
      scalars[field_idx] = evaluate_polynomial(&coefficients, x);
    }

    shares.push(ThresholdCertificateShare::from_scalars(index, scalars));
  }

  Ok(shares)
}

pub fn recover_threshold_certificate(
  shares: &[ThresholdCertificateShare],
  threshold: usize,
) -> Result<MiniCertificate, String> {
  if shares.len() < threshold {
    return Err(format!(
      "need at least {threshold} shares, got {}",
      shares.len()
    ));
  }

  let selected = &shares[.. threshold];
  validate_unique_indices(selected)?;

  let recovered = [0, 1, 2].map(|field_idx| {
    let points = selected
      .iter()
      .map(|share| {
        (
          MpcScalar::from(share.index),
          (*share).scalar_at_field(field_idx),
        )
      })
      .collect::<Vec<_>>();

    interpolate_at_zero(&points)
  });

  Ok(MiniCertificate::from_scalars(recovered))
}

pub fn run_additive_certificate_example() -> MiniCertificate {
  let certificate = MiniCertificate::new(2026, 7, 42);
  let private_share = AdditiveCertificateShare::from_scalars([
    MpcScalar::from(11u64),
    MpcScalar::from(13u64),
    MpcScalar::from(17u64),
  ]);
  let (private_share, public_share) = split_certificate_additively(certificate, private_share);

  let recovered = private_share.add_shares(public_share);
  assert_eq!(recovered, certificate);

  recovered
}

pub fn run_threshold_certificate_example() -> ThresholdCertificateExample {
  let certificate = MiniCertificate::new(2026, 7, 42);
  let threshold = 3;
  let total_shares = 5;
  let shares = split_certificate_threshold(certificate, total_shares, threshold).unwrap();
  let selected = [shares[0], shares[2], shares[4]];
  let recovered = recover_threshold_certificate(&selected, threshold).unwrap();

  assert_eq!(recovered, certificate);

  ThresholdCertificateExample {
    threshold,
    total_shares,
    used_indices: selected.iter().map(|share| share.index).collect(),
    recovered,
  }
}

fn deterministic_polynomial(
  secret: MpcScalar,
  field_idx: usize,
  threshold: usize,
) -> Vec<MpcScalar> {
  let mut coefficients = Vec::with_capacity(threshold);
  coefficients.push(secret);

  for degree in 1 .. threshold {
    let coefficient = 100 + (field_idx as u64 + 1) * 17 + degree as u64 * 29;
    coefficients.push(MpcScalar::from(coefficient));
  }

  coefficients
}

fn evaluate_polynomial(coefficients: &[MpcScalar], x: MpcScalar) -> MpcScalar {
  coefficients
    .iter()
    .rev()
    .fold(MpcScalar::from(0u64), |acc, coefficient| {
      acc * x + *coefficient
    })
}

fn interpolate_at_zero(points: &[(MpcScalar, MpcScalar)]) -> MpcScalar {
  let mut recovered = MpcScalar::from(0u64);

  for (i, (x_i, y_i)) in points.iter().copied().enumerate() {
    let mut basis = MpcScalar::from(1u64);

    for (j, (x_j, _)) in points.iter().copied().enumerate() {
      if i == j {
        continue;
      }

      basis = basis * (MpcScalar::from(0u64) - x_j) / (x_i - x_j);
    }

    recovered += y_i * basis;
  }

  recovered
}

fn validate_unique_indices(shares: &[ThresholdCertificateShare]) -> Result<(), String> {
  let mut seen = HashSet::with_capacity(shares.len());

  for share in shares {
    if share.index == 0 {
      return Err("share index must be nonzero".to_string());
    }
    if !seen.insert(share.index) {
      return Err(format!("duplicate share index {}", share.index));
    }
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn additive_certificate_reconstructs_with_both_shares() {
    assert_eq!(
      run_additive_certificate_example(),
      MiniCertificate::new(2026, 7, 42)
    );
  }

  #[test]
  fn threshold_certificate_reconstructs_from_three_of_five() {
    let example = run_threshold_certificate_example();

    assert_eq!(example.threshold, 3);
    assert_eq!(example.total_shares, 5);
    assert_eq!(example.used_indices, vec![1, 3, 5]);
    assert_eq!(example.recovered, MiniCertificate::new(2026, 7, 42));
  }

  #[test]
  fn threshold_certificate_reconstructs_from_a_different_subset() {
    let certificate = MiniCertificate::new(2026, 7, 42);
    let shares = split_certificate_threshold(certificate, 5, 3).unwrap();
    let selected = [shares[1], shares[2], shares[3]];

    assert_eq!(
      recover_threshold_certificate(&selected, 3).unwrap(),
      certificate
    );
  }

  #[test]
  fn threshold_certificate_rejects_too_few_shares() {
    let certificate = MiniCertificate::new(2026, 7, 42);
    let shares = split_certificate_threshold(certificate, 5, 3).unwrap();

    let err = recover_threshold_certificate(&shares[.. 2], 3).unwrap_err();
    assert!(err.contains("need at least 3 shares"));
  }
}
