//! Demonstrates trait mocking with `mockall`.
//!
//! The production code depends on a trait. Tests replace that dependency with a
//! generated mock and assert the interaction contract.

/// Reads account balances from an external dependency.
pub trait BalanceStore {
  /// Returns the balance in cents for `account_id`.
  fn balance_cents(&self, account_id: u64) -> Option<i64>;
}

/// Returns true when the account has enough funds for a debit.
#[must_use]
pub fn can_debit(store: &dyn BalanceStore, account_id: u64, amount_cents: i64) -> bool {
  store
    .balance_cents(account_id)
    .is_some_and(|balance| balance >= amount_cents)
}

#[cfg(test)]
mod tests {
  use mockall::{mock, predicate::eq};

  use super::*;

  mock! {
      Store {}

      impl BalanceStore for Store {
          fn balance_cents(&self, account_id: u64) -> Option<i64>;
      }
  }

  #[test]
  fn can_debit_when_balance_is_large_enough() {
    let mut store = MockStore::new();
    store
      .expect_balance_cents()
      .with(eq(42))
      .times(1)
      .return_const(Some(10_000));

    assert!(can_debit(&store, 42, 2_500));
  }

  #[test]
  fn cannot_debit_missing_account() {
    let mut store = MockStore::new();
    store
      .expect_balance_cents()
      .with(eq(7))
      .times(1)
      .return_const(None);

    assert!(!can_debit(&store, 7, 500));
  }
}
