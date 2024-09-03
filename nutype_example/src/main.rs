use nutype::nutype;

// Define a newtype `Name` with custom validation logic and a custom error type `NameError`.
// If validation fails, `Name` cannot be instantiated.
#[nutype(
  validate(with = validate_name, error = NameError),
  derive(Debug, AsRef, PartialEq),
)]
struct Name(String);

// Custom error type for `Name` validation.
// You can use `thiserror` or similar crates to provide more detailed error messages.
#[derive(Debug, PartialEq)]
enum NameError {
  TooShort { min: usize, length: usize },
  TooLong { max: usize, length: usize },
}

// Validation function for `Name` that checks its length.
fn validate_name(name: &str) -> Result<(), NameError> {
  const MIN: usize = 3;
  const MAX: usize = 10;
  let length = name.encode_utf16().count();

  if length < MIN {
    Err(NameError::TooShort { min: MIN, length })
  } else if length > MAX {
    Err(NameError::TooLong { max: MAX, length })
  } else {
    Ok(())
  }
}

fn main() {
  // Example usage: attempt to create a `Name` instance with an invalid value.
  assert_eq!(
    Name::try_new("Fo"),
    Err(NameError::TooShort { min: 3, length: 2 })
  );
}
