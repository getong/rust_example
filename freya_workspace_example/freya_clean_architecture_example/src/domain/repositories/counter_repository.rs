use std::{error::Error, fmt};

use crate::domain::entities::Counter;

pub(crate) trait CounterRepository {
  fn load(&self) -> Result<Option<Counter>, CounterRepositoryError>;

  fn save(&self, counter: &Counter) -> Result<(), CounterRepositoryError>;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CounterRepositoryError {
  message: String,
}

impl CounterRepositoryError {
  pub(crate) fn new(message: impl Into<String>) -> Self {
    Self {
      message: message.into(),
    }
  }
}

impl fmt::Display for CounterRepositoryError {
  fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str(&self.message)
  }
}

impl Error for CounterRepositoryError {}
