use serde::Deserialize;
use validator::{Validate, ValidationError};

fn validate_unique_username(username: &str) -> Result<(), ValidationError> {
  if username == "xXxShad0wxXx" {
    return Err(ValidationError::new("terrible_username"));
  }

  Ok(())
}

fn validate_signup(data: &SignupData) -> Result<(), ValidationError> {
  if data.mail.ends_with("gmail.com") && data.age == 18 {
    return Err(ValidationError::new("stupid_rule"));
  }

  Ok(())
}

#[allow(unused)]
#[derive(Debug, Validate, Deserialize)]
#[validate(schema(function = validate_signup))]
struct SignupData {
  #[validate(email)]
  mail: String,
  #[validate(url)]
  site: String,
  #[validate(length(min = 1), custom(function = validate_unique_username))]
  #[serde(rename = "firstName")]
  first_name: String,
  #[validate(range(min = 18, max = 20))]
  age: u32,
  #[validate(nested)]
  card: Option<Card>,
  #[validate(nested)]
  preferences: Vec<Preference>,
}

#[derive(Debug, Validate, Deserialize)]
struct Card {
  #[validate(credit_card)]
  number: String,
  #[validate(range(min = 100, max = 9999))]
  cvv: u32,
}

#[allow(dead_code)]
#[derive(Debug, Validate, Deserialize)]
struct Preference {
  #[validate(length(min = 4))]
  name: String,
  value: bool,
}

fn main() {
  // println!("Hello, world!");
  let signup = SignupData {
    mail: "bob@bob.com".to_string(),
    site: "http://hello.com".to_string(),
    first_name: "Bob".to_string(),
    age: 18,
    card: Some(Card {
      number: "5236313877109142".to_string(),
      cvv: 123,
    }),
    preferences: vec![Preference {
      name: "marketing".to_string(),
      value: false,
    }],
  };

  assert!(signup.validate().is_ok());
}
