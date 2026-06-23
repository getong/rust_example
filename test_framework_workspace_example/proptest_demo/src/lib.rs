//! Demonstrates `proptest` property-based testing.
//!
//! `proptest` generates many structured inputs and shrinks failures to a
//! smaller counterexample, which is useful for validating invariants.

/// Returns a sorted copy of `values`.
#[must_use]
pub fn sorted_copy(values: &[i32]) -> Vec<i32> {
  let mut sorted = values.to_vec();
  sorted.sort_unstable();
  sorted
}

/// Reverses text by Unicode scalar values.
#[must_use]
pub fn reverse_chars(input: &str) -> String {
  input.chars().rev().collect()
}

#[cfg(test)]
mod tests {
  use proptest::prelude::*;

  use super::*;

  proptest! {
      #[test]
      fn sorting_preserves_length(values in proptest::collection::vec(any::<i32>(), 0..128)) {
          let sorted = sorted_copy(&values);

          prop_assert_eq!(sorted.len(), values.len());
      }

      #[test]
      fn sorting_orders_items(values in proptest::collection::vec(any::<i32>(), 0..128)) {
          let sorted = sorted_copy(&values);

          prop_assert!(sorted.windows(2).all(|pair| pair[0] <= pair[1]));
      }

      #[test]
      fn reversing_twice_returns_original(input in "\\PC*") {
          let reversed_twice = reverse_chars(&reverse_chars(&input));

          prop_assert_eq!(reversed_twice, input);
      }
  }
}
