use std::{
  fs::File,
  io::{Error, ErrorKind, Write},
};

use google_authenticator::{ErrorCorrectionLevel, GoogleAuthenticator};

fn main() -> std::io::Result<()> {
  let secret = "I3VFM3JKMNDJCDH5BMBEEQAW6KJ6NOE3";
  let auth = GoogleAuthenticator::new();
  // let secret = auth.create_secret(32);
  let code = auth.get_code(&secret, 0).unwrap();

  assert!(auth.verify_code(&secret, &code, 1, 0));

  // make qr code http url
  let auth = GoogleAuthenticator::new();
  let secret = "I3VFM3JKMNDJCDH5BMBEEQAW6KJ6NOE3";
  println!(
    "{}",
    auth.qr_code_url(
      secret,
      "qr_code",
      "name",
      200,
      200,
      ErrorCorrectionLevel::High
    )
  );

  // make svg format file
  let secret = "I3VFM3JKMNDJCDH5BMBEEQAW6KJ6NOE3";
  let auth = GoogleAuthenticator::new();

  if let Ok(qr_code_string) = auth.qr_code(
    secret,
    "qr_code",
    "name",
    200,
    200,
    ErrorCorrectionLevel::High,
  ) {
    let mut file = File::create("qr_code.svg")?;
    return Ok(file.write_all(&qr_code_string.as_bytes()).unwrap());
  }

  // errors can be created from strings
  let custom_error = Error::new(ErrorKind::Other, "generate error");
  Err(custom_error)
}
