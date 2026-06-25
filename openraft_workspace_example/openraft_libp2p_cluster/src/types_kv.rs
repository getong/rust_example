//! Shared KV request/response types for this crate.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A request to the KV store.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Request {
  Set { key: String, value: String },
  Delete { key: String },
}

impl Request {
  pub fn set(key: impl Into<String>, value: impl Into<String>) -> Self {
    Request::Set {
      key: key.into(),
      value: value.into(),
    }
  }

  pub fn delete(key: impl Into<String>) -> Self {
    Request::Delete { key: key.into() }
  }
}

impl fmt::Display for Request {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Request::Set { key, value } => write!(f, "Set {{ key: {}, value: {} }}", key, value),
      Request::Delete { key } => write!(f, "Delete {{ key: {} }}", key),
    }
  }
}

/// A response from the KV store.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
  pub value: Option<String>,
}

impl Response {
  pub fn new(value: impl Into<String>) -> Self {
    Response {
      value: Some(value.into()),
    }
  }

  pub fn none() -> Self {
    Response { value: None }
  }
}
