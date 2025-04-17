use std::panic::Location;

use log::error;
use sqlx::Error as SqlxError;

#[derive(Debug)]
pub enum Error {
  DB {
    code: u32,
    source: SqlxError,
    file: &'static str,
    line: u32,
  },
  // Other variants can be added here...
}

impl Error {
  #[track_caller]
  pub fn db(code: u32, source: SqlxError) -> Self {
    let location = Location::caller();
    error!(
      "database error at {}:{} - {}",
      location.file(),
      location.line(),
      source
    );
    Self::DB {
      code,
      source,
      file: location.file(),
      line: location.line(),
    }
  }
}

fn main() {
  // Initialize logging (env_logger is common and simple)
  env_logger::init();

  // Simulate a database error and wrap it
  let simulated_error = SqlxError::RowNotFound;
  let my_error = Error::db(404, simulated_error);

  // Print it to demonstrate
  println!("Custom error: {:?}", my_error);
}
