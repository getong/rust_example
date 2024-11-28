// use std::fmt::Error;
use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DataStoreError {
  #[error("data store disconnected")]
  Disconnect(#[from] io::Error),
  #[error("the data for key `{0}` is not available")]
  Redaction(String),
  #[error("invalid header (expected {expected:?}, found {found:?})")]
  InvalidHeader { expected: String, found: String },
  #[error("unknown data store error")]
  Unknown,
}

fn main() {
  // println!("Hello, world!");
  let store_error = DataStoreError::Redaction("wrong connection".to_string());

  // store_error:Redaction("wrong connection")
  println!("store_error:{:?}", store_error);
  // store_error:the data for key `wrong connection` is not available
  println!("store_error:{}", store_error);

  let store_error = DataStoreError::InvalidHeader {
    expected: "a".to_string(),
    found: "b".to_string(),
  };
  println!("store_error:{:?}", store_error);
  println!("store_error:{}", store_error);
}
