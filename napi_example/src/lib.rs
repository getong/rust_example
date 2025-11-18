#![deny(clippy::all)]

use std::time::{SystemTime, UNIX_EPOCH};

use napi::bindgen_prelude::{Error, Result, Status};
use napi_derive::napi;

#[napi]
pub fn version() -> String {
  env!("CARGO_PKG_VERSION").to_string()
}

#[napi]
pub fn add(left: i64, right: i64) -> i64 {
  left + right
}

#[napi]
pub fn repeat(message: String, times: u32) -> String {
  if times == 0 {
    String::new()
  } else {
    message.repeat(times as usize)
  }
}

#[napi]
pub fn average(values: Vec<f64>) -> Result<f64> {
  if values.is_empty() {
    return Err(Error::new(
      Status::InvalidArg,
      "average function requires at least one value".to_string(),
    ));
  }

  let sum: f64 = values.iter().sum();
  Ok(sum / values.len() as f64)
}

#[napi(object)]
pub struct Summary {
  pub input: Vec<i64>,
  pub sum: i64,
  pub calculated_at: i64,
}

#[napi]
pub fn summarize(values: Vec<i64>) -> Result<Summary> {
  if values.is_empty() {
    return Err(Error::new(
      Status::InvalidArg,
      "summarize function requires at least one value".to_string(),
    ));
  }

  let sum = values.iter().copied().sum();
  let calculated_at = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_err(|err| Error::from_reason(err.to_string()))?
    .as_secs() as i64;

  Ok(Summary {
    input: values,
    sum,
    calculated_at,
  })
}

#[napi]
pub struct Greeter {
  greeting: String,
}

#[napi]
impl Greeter {
  #[napi(constructor)]
  pub fn new(greeting: Option<String>) -> Self {
    Self {
      greeting: greeting.unwrap_or_else(|| "Hello".to_string()),
    }
  }

  #[napi(getter)]
  pub fn greeting(&self) -> String {
    self.greeting.clone()
  }

  #[napi(setter)]
  pub fn set_greeting(&mut self, value: String) {
    self.greeting = value;
  }

  #[napi]
  pub fn greet(&self, recipient: String) -> String {
    format!("{} {}!", self.greeting, recipient)
  }

  #[napi]
  pub fn greet_many(&self, recipients: Vec<String>) -> Vec<String> {
    recipients
      .into_iter()
      .map(|name| self.greet(name))
      .collect()
  }
}
