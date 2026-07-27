use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::{
  entities::{Counter, CounterId},
  repositories::{CounterRepository, CounterRepositoryError},
};

#[derive(Debug)]
pub(crate) struct JsonFileCounterRepository {
  path: PathBuf,
}

impl JsonFileCounterRepository {
  pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
    Self { path: path.into() }
  }

  fn error(&self, operation: &str, error: impl std::fmt::Display) -> CounterRepositoryError {
    CounterRepositoryError::new(format!(
      "could not {operation} {}: {error}",
      self.path.display()
    ))
  }
}

impl CounterRepository for JsonFileCounterRepository {
  fn load(&self) -> Result<Option<Counter>, CounterRepositoryError> {
    let json = match fs::read_to_string(&self.path) {
      Ok(json) => json,
      Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(self.error("read", error)),
    };

    let stored: StoredCounter =
      serde_json::from_str(&json).map_err(|error| self.error("decode", error))?;

    Ok(Some(Counter::new(CounterId::new(stored.id), stored.value)))
  }

  fn save(&self, counter: &Counter) -> Result<(), CounterRepositoryError> {
    if let Some(parent) = self
      .path
      .parent()
      .filter(|path| !path.as_os_str().is_empty())
    {
      fs::create_dir_all(parent).map_err(|error| self.error("create directory for", error))?;
    }

    let json = serde_json::to_string_pretty(&StoredCounter::from(counter))
      .map_err(|error| self.error("encode", error))?;

    fs::write(&self.path, json).map_err(|error| self.error("write", error))
  }
}

#[derive(Debug, Deserialize, Serialize)]
struct StoredCounter {
  id: u64,
  value: i32,
}

impl From<&Counter> for StoredCounter {
  fn from(counter: &Counter) -> Self {
    Self {
      id: counter.id().value(),
      value: counter.value(),
    }
  }
}

#[cfg(test)]
mod tests {
  use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
  };

  use super::*;

  fn test_path(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system time should be after the Unix epoch")
      .as_nanos();

    std::env::temp_dir().join(format!(
      "freya-clean-architecture-{test_name}-{}-{nonce}.json",
      std::process::id()
    ))
  }

  fn remove_test_file(path: &Path) {
    if let Err(error) = fs::remove_file(path) {
      assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }
  }

  #[test]
  fn load_returns_none_when_the_file_does_not_exist() {
    let path = test_path("missing");
    let repository = JsonFileCounterRepository::new(&path);

    let counter = repository
      .load()
      .expect("a missing persistence file should not be an error");

    assert!(counter.is_none());
  }

  #[test]
  fn save_and_load_round_trip_the_counter() {
    let path = test_path("round-trip");
    let repository = JsonFileCounterRepository::new(&path);
    let expected = Counter::new(CounterId::new(9), 42);

    repository
      .save(&expected)
      .expect("counter should be written to the temporary file");
    let actual = repository
      .load()
      .expect("counter should be read from the temporary file")
      .expect("the temporary file should contain a counter");

    assert_eq!(actual, expected);
    assert_eq!(actual.value(), 42);
    remove_test_file(&path);
  }

  #[test]
  fn load_reports_invalid_json() {
    let path = test_path("invalid");
    fs::write(&path, "not-json").expect("invalid fixture should be written");
    let repository = JsonFileCounterRepository::new(&path);

    let error = repository
      .load()
      .expect_err("invalid JSON should be reported");

    assert!(error.to_string().contains("could not decode"));
    remove_test_file(&path);
  }
}
