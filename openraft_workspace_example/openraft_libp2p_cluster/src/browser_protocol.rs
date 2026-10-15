//! HTTP response shared with the independent GPUI client via a source module.
//! Keep this module storage/runtime independent: the client only needs Serde.
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Row {
  pub key: String,
  pub value: String,
  pub key_bytes: usize,
  pub value_bytes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Page {
  pub group_id: String,
  pub column_families: Vec<String>,
  pub column_family: String,
  pub rows: Vec<Row>,
  /// Opaque, URL-safe cursor. Clients must not derive it from preview text.
  pub next: Option<String>,
}
