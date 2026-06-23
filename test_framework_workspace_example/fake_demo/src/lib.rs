//! Demonstrates generating test data with `fake`.

use fake::{
  Dummy, Fake, Faker,
  faker::{
    internet::en::{FreeEmail, Username},
    name::en::Name,
  },
};

/// A sample user fixture.
#[derive(Debug, Clone, PartialEq, Eq, Dummy)]
pub struct TestUser {
  /// Generated display name.
  #[dummy(faker = "Name()")]
  pub name: String,
  /// Generated email address.
  #[dummy(faker = "FreeEmail()")]
  pub email: String,
  /// Generated username.
  #[dummy(faker = "Username()")]
  pub username: String,
}

/// Generates one fake user.
#[must_use]
pub fn fake_user() -> TestUser {
  Faker.fake()
}

/// Generates `count` fake users.
#[must_use]
pub fn fake_users(count: usize) -> Vec<TestUser> {
  (0 .. count).map(|_| fake_user()).collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn fake_user_populates_fields() {
    let user = fake_user();

    assert!(!user.name.is_empty());
    assert!(user.email.contains('@'));
    assert!(!user.username.is_empty());
  }

  #[test]
  fn fake_users_returns_requested_count() {
    assert_eq!(fake_users(5).len(), 5);
  }
}
