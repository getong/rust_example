//! Demonstrates code suitable for benchmarking with `criterion`.

/// Sorts a copy of the input data.
#[must_use]
pub fn sorted_copy(values: &[u64]) -> Vec<u64> {
  let mut sorted = values.to_vec();
  sorted.sort_unstable();
  sorted
}

/// Builds deterministic sample data for repeatable benchmarks.
#[must_use]
pub fn sample_values(len: usize) -> Vec<u64> {
  (0 .. len)
    .map(|index| {
      let mixed = (index as u64)
        .wrapping_mul(6364136223846793005_u64)
        .wrapping_add(1442695040888963407_u64);
      mixed ^ (mixed >> 33)
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sorted_copy_orders_values() {
    let sorted = sorted_copy(&[9, 1, 4, 1]);

    assert_eq!(sorted, [1, 1, 4, 9]);
  }

  #[test]
  fn sample_values_has_requested_length() {
    assert_eq!(sample_values(32).len(), 32);
  }
}
