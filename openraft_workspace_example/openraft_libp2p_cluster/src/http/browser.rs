//! Bounded, local-node inspection over HTTP, using the node's existing DB handle.
use std::sync::Arc;

use anyhow::Context;
use axum::{
  extract::{Query, State},
  http::StatusCode,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rocksdb::{DB, Direction, IteratorMode};
use serde::Deserialize;

use super::{AppState, Json};
use crate::browser_protocol::{Page, Row};

const PAGE_SIZE: usize = 100;
const PREVIEW_BYTES: usize = 4096;
const COLUMN_FAMILIES: [&str; 5] = ["default", "sm_data", "sm_meta", "meta", "logs"];

#[derive(Debug, Default, Deserialize)]
pub struct BrowseQuery {
  #[serde(alias = "group")]
  pub group_id: Option<String>,
  pub cf: Option<String>,
  #[serde(default)]
  pub prefix: String,
  pub cursor: Option<String>,
}

impl BrowseQuery {
  fn validate(&self) -> Result<Option<Vec<u8>>, (StatusCode, String)> {
    if !COLUMN_FAMILIES.contains(&self.cf.as_deref().unwrap_or("sm_data")) {
      return Err((StatusCode::BAD_REQUEST, "unknown column family".into()));
    }
    let after = self
      .cursor
      .as_ref()
      .map(|cursor| URL_SAFE_NO_PAD.decode(cursor))
      .transpose()
      .map_err(|_| (StatusCode::BAD_REQUEST, "invalid cursor".into()))?;
    if after
      .as_ref()
      .is_some_and(|key| !key.starts_with(self.prefix.as_bytes()))
    {
      return Err((
        StatusCode::BAD_REQUEST,
        "cursor does not match prefix".into(),
      ));
    }
    Ok(after)
  }
}

pub async fn entries(
  State(state): State<Arc<AppState>>,
  Query(query): Query<BrowseQuery>,
) -> Result<Json<Page>, (StatusCode, String)> {
  let after = query.validate()?;
  let group_id = query
    .group_id
    .clone()
    .filter(|id| !id.is_empty())
    .unwrap_or_else(|| state.default_group.clone());
  let group = state.registry.get(&group_id).ok_or_else(|| {
    (
      StatusCode::NOT_FOUND,
      format!("group not hosted on this node: {group_id}"),
    )
  })?;
  let db = group.kv_data.primary_db();
  let result = tokio::task::spawn_blocking(move || read_page(&db, group_id, query, after))
    .await
    .context("join RocksDB browse task")
    .and_then(|result| result);
  result.map(Json).map_err(|error| {
    tracing::warn!(%error, "rocksdb HTTP browse failed");
    (
      StatusCode::INTERNAL_SERVER_ERROR,
      "failed to read node database".into(),
    )
  })
}

/// Called only from a blocking worker. No filesystem paths come from clients.
pub(crate) fn read_page(
  db: &DB,
  group_id: String,
  query: BrowseQuery,
  after: Option<Vec<u8>>,
) -> anyhow::Result<Page> {
  let column_family = query.cf.unwrap_or_else(|| "sm_data".into());
  let cf = db
    .cf_handle(&column_family)
    .context("column family unavailable")?;
  let column_families = COLUMN_FAMILIES
    .iter()
    .filter(|name| db.cf_handle(name).is_some())
    .map(|name| (*name).to_owned())
    .collect();
  let snapshot = db.snapshot();
  let start = after.as_deref().unwrap_or(query.prefix.as_bytes());
  let mut rows = Vec::with_capacity(PAGE_SIZE);
  let mut last_key = None;
  let mut next = None;
  for item in snapshot.iterator_cf(cf, IteratorMode::From(start, Direction::Forward)) {
    let (key, value) = item.context("iterate node database")?;
    if !key.starts_with(query.prefix.as_bytes()) {
      break;
    }
    if after
      .as_deref()
      .is_some_and(|cursor| key.as_ref() <= cursor)
    {
      continue;
    }
    if rows.len() == PAGE_SIZE {
      next = last_key.map(|key: Vec<u8>| URL_SAFE_NO_PAD.encode(key));
      break;
    }
    rows.push(Row {
      key: preview(&key),
      value: preview(&value),
      key_bytes: key.len(),
      value_bytes: value.len(),
    });
    last_key = Some(key.to_vec());
  }
  Ok(Page {
    group_id,
    column_families,
    column_family,
    rows,
    next,
  })
}

fn preview(bytes: &[u8]) -> String {
  let mut end = bytes.len().min(PREVIEW_BYTES);
  // Avoid mislabeling UTF-8 as binary if the preview ends inside a code point.
  if let Err(error) = std::str::from_utf8(&bytes[.. end]) {
    if error.error_len().is_none() && bytes.len() > end {
      end = error.valid_up_to();
    }
  }
  let mut result = match std::str::from_utf8(&bytes[.. end]) {
    Ok(text)
      if !text
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t') =>
    {
      text.to_owned()
    }
    _ => {
      use std::fmt::Write;
      let mut text = String::from("hex: ");
      for byte in &bytes[.. end] {
        let _ = write!(text, "{byte:02x}");
      }
      text
    }
  };
  if bytes.len() > end {
    result.push_str(&format!(" … [仅预览前 {end} / {} 字节]", bytes.len()));
  }
  result
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn pagination_reads_primary_and_preserves_binary_cursor() -> anyhow::Result<()> {
    let dir = tempfile::tempdir()?;
    let mut options = rocksdb::Options::default();
    options.create_if_missing(true);
    options.create_missing_column_families(true);
    let db = DB::open_cf(&options, dir.path(), ["sm_data", "logs"])?;
    let cf = db.cf_handle("sm_data").unwrap();
    for i in 0 .. 105u8 {
      db.put_cf(cf, [b't', i, 255], b"value")?;
    }
    db.put_cf(cf, b"other", b"excluded")?;
    let query = BrowseQuery {
      prefix: "t".into(),
      ..Default::default()
    };
    let first = read_page(&db, "users".into(), query, None)?;
    assert_eq!(first.rows.len(), 100);
    assert_eq!(first.rows[0].key, "hex: 7400ff");
    let query = BrowseQuery {
      prefix: "t".into(),
      cursor: first.next,
      ..Default::default()
    };
    let after = query.validate().unwrap();
    assert_eq!(after.as_deref(), Some([b't', 99, 255].as_slice()));
    db.put_cf(cf, [b't', 105, 255], b"new")?;
    let second = read_page(&db, "users".into(), query, after)?;
    assert_eq!(second.rows.len(), 6);
    assert!(second.next.is_none());
    let empty = read_page(
      &db,
      "users".into(),
      BrowseQuery {
        cf: Some("logs".into()),
        ..Default::default()
      },
      None,
    )?;
    assert!(empty.rows.is_empty());
    Ok(())
  }

  #[test]
  fn validates_query_and_bounds_previews() {
    for query in [
      BrowseQuery {
        cf: Some("missing".into()),
        ..Default::default()
      },
      BrowseQuery {
        cursor: Some("!".into()),
        ..Default::default()
      },
      BrowseQuery {
        prefix: "a".into(),
        cursor: Some(URL_SAFE_NO_PAD.encode(b"b")),
        ..Default::default()
      },
    ] {
      assert_eq!(query.validate().unwrap_err().0, StatusCode::BAD_REQUEST);
    }
    assert_eq!(preview("你好".as_bytes()), "你好");
    assert_eq!(preview(&[0, 255]), "hex: 00ff");
    assert!(preview(&vec![b'a'; 5000]).contains("4096 / 5000"));
    assert!(!preview("你".repeat(2000).as_bytes()).starts_with("hex:"));
  }
}
