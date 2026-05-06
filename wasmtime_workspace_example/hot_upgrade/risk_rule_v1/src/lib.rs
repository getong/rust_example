/// Returns the required state schema version for this rule.
/// The host service will migrate State to this schema before calling `decide`.
#[unsafe(no_mangle)]
pub extern "C" fn required_schema() -> i32 {
  1
}

/// Evaluates a risk decision.
///
/// # Arguments
/// * `_user_id` – the requesting user's ID (unused in v1)
/// * `amount`   – transaction amount
///
/// # Returns
/// * `0` – allow
/// * `1` – review
#[unsafe(no_mangle)]
pub extern "C" fn decide(_user_id: i64, amount: i64) -> i32 {
  if amount > 10_000 {
    1 // review
  } else {
    0 // allow
  }
}
