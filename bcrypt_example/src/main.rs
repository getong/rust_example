use bcrypt::{DEFAULT_COST, hash, verify};

fn main() -> Result<(), bcrypt::BcryptError> {
  let password = "correct horse battery staple";
  let wrong_password = "tr0ub4dor&3";

  let password_hash = hash(password, DEFAULT_COST)?;

  println!("Password: {password}");
  println!("Bcrypt hash: {password_hash}");
  println!(
    "Matches original password: {}",
    verify(password, &password_hash)?
  );
  println!(
    "Matches wrong password: {}",
    verify(wrong_password, &password_hash)?
  );

  Ok(())
}
