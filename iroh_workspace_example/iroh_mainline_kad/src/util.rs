use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) fn now_unix_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
}

pub(crate) fn now_unix_nanos() -> u128 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_nanos()
}

pub(crate) fn new_nonce() -> u64 {
  let nanos = now_unix_nanos();
  let mixed = nanos ^ nanos.rotate_left(64) ^ u128::from(std::process::id());
  mixed as u64 ^ (mixed >> 64) as u64
}

pub(crate) fn display_values(values: &[String]) -> String {
  if values.is_empty() {
    "-".to_string()
  } else {
    values.join(" ")
  }
}

/// Exponential backoff: base_ms * 2^attempt
pub(crate) fn backoff_duration(base_ms: u64, attempt: u32) -> Duration {
  Duration::from_millis(base_ms * 2u64.pow(attempt))
}

/// Format bytes as lowercase hex string.
pub(crate) fn hex_encode(data: &[u8]) -> String {
  data.iter().map(|b| format!("{b:02x}")).collect()
}
