#![deny(clippy::all)]

use std::{
  cell::RefCell,
  time::{SystemTime, UNIX_EPOCH},
};

use napi::bindgen_prelude::{Error, FnArgs, Function, Result, Status};
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

#[napi]
pub fn run_typescript_callback(
  callback: Function<String, String>,
  topic: String,
) -> Result<String> {
  run_typescript_callback_logic(topic, |payload| callback.call(payload))
}

#[napi]
pub fn emit_typescript_events(callback: Function<String, ()>, values: Vec<String>) -> Result<()> {
  emit_typescript_events_logic(values, |payload| callback.call(payload))
}

fn run_typescript_callback_logic<F>(topic: String, mut callback: F) -> Result<String>
where
  F: FnMut(String) -> Result<String>,
{
  let message = format!("Rust received request for {topic}");
  callback(message)
}

fn emit_typescript_events_logic<F>(values: Vec<String>, mut callback: F) -> Result<()>
where
  F: FnMut(String) -> Result<()>,
{
  for (index, value) in values.into_iter().enumerate() {
    let payload = format!("event #{index}: {value}");
    callback(payload)?;
  }

  Ok(())
}

pub fn run_typescript_callback_example(topic: String) -> Result<String> {
  run_typescript_callback_logic(topic, |payload| Ok(payload.to_uppercase()))
}

pub fn emit_typescript_events_example(values: Vec<String>) -> Result<Vec<String>> {
  let mut collected = Vec::new();
  emit_typescript_events_logic(values, |payload| {
    collected.push(payload);
    Ok(())
  })?;

  Ok(collected)
}

#[napi]
pub fn call_typescript_transformer(
  callback: Function<FnArgs<(String, i32)>, String>,
  input: String,
  multiplier: i32,
) -> Result<String> {
  call_typescript_transformer_logic(input, multiplier, |payload, times| {
    callback.call((payload, times).into())
  })
}

#[napi]
pub fn orchestrate_typescript_decision(
  ask: Function<String, bool>,
  on_approved: Function<String, ()>,
  on_denied: Function<String, ()>,
  topic: String,
) -> Result<bool> {
  orchestrate_typescript_decision_logic(
    topic,
    |payload| ask.call(payload),
    |payload| on_approved.call(payload),
    |payload| on_denied.call(payload),
  )
}

fn call_typescript_transformer_logic<F>(
  input: String,
  multiplier: i32,
  mut transformer: F,
) -> Result<String>
where
  F: FnMut(String, i32) -> Result<String>,
{
  let stage_one = transformer(format!("stage-one: {input}"), multiplier)?;
  transformer(format!("stage-two: {stage_one}"), 1)
}

fn orchestrate_typescript_decision_logic<FAsk, FApproved, FDenied>(
  topic: String,
  mut ask: FAsk,
  mut on_approved: FApproved,
  mut on_denied: FDenied,
) -> Result<bool>
where
  FAsk: FnMut(String) -> Result<bool>,
  FApproved: FnMut(String) -> Result<()>,
  FDenied: FnMut(String) -> Result<()>,
{
  let decision = ask(topic.clone())?;
  if decision {
    on_approved(format!("Rust confirmed {topic}"))?;
  } else {
    on_denied(format!("Rust rejected {topic}"))?;
  }

  Ok(decision)
}

pub fn call_typescript_transformer_example(input: String, multiplier: i32) -> Result<String> {
  call_typescript_transformer_logic(input, multiplier, |payload, times| {
    Ok(format!("simulated TS transform: {payload} x{times}"))
  })
}

pub struct DecisionSummary {
  pub decision: bool,
  pub log: Vec<String>,
}

pub fn orchestrate_typescript_decision_example(topic: String) -> Result<DecisionSummary> {
  let log = RefCell::new(Vec::new());
  let decision = orchestrate_typescript_decision_logic(
    topic,
    |subject| {
      log
        .borrow_mut()
        .push(format!("ask callback received topic: {subject}"));
      Ok(subject.chars().count() % 2 == 0)
    },
    |message| {
      log
        .borrow_mut()
        .push(format!("approved callback message: {message}"));
      Ok(())
    },
    |message| {
      log
        .borrow_mut()
        .push(format!("denied callback message: {message}"));
      Ok(())
    },
  )?;

  Ok(DecisionSummary {
    decision,
    log: log.into_inner(),
  })
}
