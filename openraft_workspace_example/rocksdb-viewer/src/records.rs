//! Presentation model: JSON values become scalar fields, never raw JSON cells.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::Row;

const MAX_FIELDS: usize = 128;
const MAX_DEPTH: usize = 16;

#[derive(Default)]
pub struct Records {
  pub columns: Vec<String>,
  pub rows: Vec<Record>,
}

pub struct Record {
  pub key: String,
  pub kind: String,
  pub fields: BTreeMap<String, String>,
  pub notice: Option<String>,
}

impl Records {
  pub fn from_rows(rows: &[Row]) -> Self {
    let rows: Vec<_> = rows.iter().map(Record::from_row).collect();
    let columns: BTreeSet<_> = rows
      .iter()
      .flat_map(|row| row.fields.keys().cloned())
      .collect();
    // The table shows common columns first; all fields remain in row details.
    let mut columns: Vec<_> = columns.into_iter().collect();
    columns.sort_by_key(|path| (priority(path), path.clone()));
    columns.truncate(24);
    Self { columns, rows }
  }
}

fn priority(path: &str) -> usize {
  match path {
    "/id" | "/task_id" | "/name" => 0,
    "/status" | "/state" | "/kind" => 1,
    "/created_at" | "/updated_at" => 2,
    _ => 3,
  }
}

impl Record {
  fn from_row(row: &Row) -> Self {
    let mut record = Self {
      key: if row.key.starts_with("hex: ") {
        format!("二进制键 · {} 字节", row.key_bytes)
      } else {
        row.key.clone()
      },
      kind: "文本".into(),
      fields: BTreeMap::new(),
      notice: None,
    };
    let text = row.value.trim();
    if text.contains(" … [仅预览前 ") {
      record.kind = "不完整".into();
      record.notice = Some(format!(
        "服务端返回的值已截断（原始 {} 字节），无法完整展开字段。",
        row.value_bytes
      ));
    } else if text.starts_with("hex: ") {
      record.kind = "二进制".into();
      record.notice = Some(format!(
        "二进制内容 · {} 字节，无法转换为业务字段。",
        row.value_bytes
      ));
    } else {
      match serde_json::from_str::<Value>(text) {
        Ok(value) => {
          record.kind = match &value {
            Value::Object(_) => "对象",
            Value::Array(_) => "列表",
            Value::String(_) => "文本",
            Value::Number(_) => "数字",
            Value::Bool(_) => "布尔",
            Value::Null => "空值",
          }
          .into();
          let mut limited = false;
          flatten(&value, "", 0, &mut record.fields, &mut limited);
          if limited {
            record.notice =
              Some("字段过多或嵌套过深，明细仅展示前 128 个字段、最多 16 层。".into());
          }
        }
        Err(_) if text.starts_with('{') || text.starts_with('[') => {
          record.kind = "格式异常".into();
          record.notice = Some("内容不是完整有效的 JSON，无法展开字段。".into());
        }
        Err(_) => {
          record.fields.insert("".into(), row.value.clone());
        }
      }
    }
    record
  }
}

// JSON Pointer paths distinguish a literal key containing '/' from nesting.
fn flatten(
  value: &Value,
  path: &str,
  depth: usize,
  fields: &mut BTreeMap<String, String>,
  limited: &mut bool,
) {
  if fields.len() >= MAX_FIELDS || depth > MAX_DEPTH {
    *limited = true;
    return;
  }
  match value {
    Value::String(text) if text.trim_start().starts_with(['{', '[']) => {
      // Some business records contain JSON encoded inside a string field.
      if let Ok(nested) = serde_json::from_str::<Value>(text) {
        flatten(&nested, path, depth + 1, fields, limited);
      } else {
        fields.insert(path.into(), "嵌入的数据格式异常".into());
      }
    }
    Value::Object(object) if !object.is_empty() => {
      for (index, (key, value)) in object.iter().enumerate() {
        let key = key.replace('~', "~0").replace('/', "~1");
        flatten(value, &format!("{path}/{key}"), depth + 1, fields, limited);
        if fields.len() >= MAX_FIELDS {
          *limited |= index + 1 < object.len();
          break;
        }
      }
    }
    Value::Array(array) if !array.is_empty() => {
      for (index, value) in array.iter().enumerate() {
        flatten(
          value,
          &format!("{path}/{index}"),
          depth + 1,
          fields,
          limited,
        );
        if fields.len() >= MAX_FIELDS {
          if index + 1 < array.len() {
            *limited = true;
          }
          break;
        }
      }
    }
    _ => {
      let text = match value {
        Value::Null => "空值".into(),
        Value::Bool(value) => if *value { "是" } else { "否" }.into(),
        Value::String(value) => {
          if value.is_empty() {
            "（空文本）".into()
          } else {
            value.clone()
          }
        }
        Value::Number(value) => value.to_string(),
        Value::Array(_) => "空列表".into(),
        Value::Object(_) => "空对象".into(),
      };
      fields.insert(path.into(), text);
    }
  }
}

pub fn label(path: &str) -> String {
  if path.is_empty() {
    return "值".into();
  }
  path
    .trim_start_matches('/')
    .split('/')
    .map(|part| {
      let decoded = part.replace("~1", "/").replace("~0", "~");
      // Quotes keep literal separators distinct from the nesting separator.
      if decoded.contains('/') || decoded.is_empty() {
        format!("「{decoded}」")
      } else {
        decoded
      }
    })
    .collect::<Vec<_>>()
    .join(" / ")
}

#[cfg(test)]
mod tests {
  use super::*;
  fn row(value: &str) -> Row {
    Row {
      key: "user:1".into(),
      value: value.into(),
      key_bytes: 6,
      value_bytes: value.len(),
    }
  }

  #[test]
  fn nested_records_have_union_columns_and_scalar_cells() {
    let records = Records::from_rows(&[
      row(r#"{"id":1,"profile":{"name":"小王"},"active":true,"tags":["a","b"],"note":null}"#),
      row(r#"{"id":2,"status":"ready"}"#),
    ]);
    assert_eq!(records.columns[0], "/id");
    assert_eq!(records.rows[0].fields["/profile/name"], "小王");
    assert_eq!(records.rows[0].fields["/tags/1"], "b");
    assert_eq!(records.rows[0].fields["/active"], "是");
    assert_eq!(records.rows[0].fields["/note"], "空值");
    assert!(!records.rows[1].fields.contains_key("/note"));
    let encoded = Record::from_row(&row(r#"{"payload":"{\"name\":\"Alice\"}"}"#));
    assert_eq!(encoded.fields["/payload/name"], "Alice");
  }

  #[test]
  fn abnormal_values_do_not_expose_raw_data() {
    for text in ["hex: 00ff", "{broken", "{ … [仅预览前 4096 / 5000 字节]"] {
      let record = Record::from_row(&row(text));
      assert!(record.fields.is_empty());
      assert!(record.notice.is_some());
    }
    assert_eq!(Record::from_row(&row("Alice")).fields[""], "Alice");
    assert_eq!(Record::from_row(&row(r#""Alice""#)).fields[""], "Alice");
  }

  #[test]
  fn paths_do_not_collide_and_large_arrays_are_bounded() {
    let record = Record::from_row(&row(r#"{"a/b":1,"a":{"b":2},"empty":[]}"#));
    assert_eq!(record.fields["/a~1b"], "1");
    assert_eq!(record.fields["/a/b"], "2");
    assert_ne!(label("/a~1b"), label("/a/b"));
    let record = Record::from_row(&row(&serde_json::to_string(&vec![1; 200]).unwrap()));
    assert_eq!(record.fields.len(), MAX_FIELDS);
    assert!(record.notice.is_some());
  }
}
