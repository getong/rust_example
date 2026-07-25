/// Returns the required state schema version for this rule.
/// Schema 2 adds explicit fast-lane hit tracking.
#[unsafe(no_mangle)]
pub extern "C" fn required_schema() -> i32 {
  2
}

/// Evaluates a risk decision.
///
/// # Arguments
/// * `user_id` – the requesting user's ID
/// * `amount`  – transaction amount
///
/// # Returns
/// * `0` – allow
/// * `1` – review
/// * `2` – allow-fast-lane (even user_id with small amount)
#[unsafe(no_mangle)]
pub extern "C" fn decide(user_id: i64, amount: i64) -> i32 {
  if amount > 5_000 {
    1 // review
  } else if user_id % 2 == 0 && amount <= 4_000 {
    2 // allow-fast-lane
  } else {
    0 // allow
  }
}
