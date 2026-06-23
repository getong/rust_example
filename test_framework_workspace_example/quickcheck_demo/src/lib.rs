//! Demonstrates `quickcheck` property-based testing.
//!
//! `quickcheck` is compact: a property is usually a normal function whose
//! argument types drive random input generation.

/// Calculates a wrapping checksum for bytes.
#[must_use]
pub fn checksum(bytes: &[u8]) -> u8 {
  bytes.iter().fold(0, |acc, byte| acc.wrapping_add(*byte))
}

/// Returns the bytes with all zero values removed.
#[must_use]
pub fn remove_zeroes(bytes: &[u8]) -> Vec<u8> {
  bytes.iter().copied().filter(|byte| *byte != 0).collect()
}

#[cfg(test)]
mod tests {
  use quickcheck_macros::quickcheck;

  use super::*;

  #[quickcheck]
  fn checksum_matches_manual_wrapping_sum(bytes: Vec<u8>) -> bool {
    let expected = bytes.iter().copied().fold(0_u8, u8::wrapping_add);

    checksum(&bytes) == expected
  }

  #[quickcheck]
  fn removing_zeroes_never_increases_length(bytes: Vec<u8>) -> bool {
    remove_zeroes(&bytes).len() <= bytes.len()
  }

  #[quickcheck]
  fn removing_zeroes_leaves_no_zero_values(bytes: Vec<u8>) -> bool {
    remove_zeroes(&bytes).iter().all(|byte| *byte != 0)
  }
}
