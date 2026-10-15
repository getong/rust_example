//! HTTP response shared with the independent GPUI client via a source module.
//! Keep this module storage/runtime independent: only Serde and JSON types.
use serde::{Deserialize, Serialize};

/// Explicit value encoding: clients decode JSON once with the HTTP envelope.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum CellValue {
  // Value is the shared representation; sonic_rs performs JSON decoding.
  Json(serde_json::Value),
  Text(String),
  Binary,
  Truncated,
  InvalidJson,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Row {
  pub key: String,
  pub value: CellValue,
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
