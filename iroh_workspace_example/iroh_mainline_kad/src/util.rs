use std::time::{SystemTime, UNIX_EPOCH};

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
