use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) fn now_unix_secs() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs()
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
