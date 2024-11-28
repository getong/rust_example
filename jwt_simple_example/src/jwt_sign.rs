// jwt_sign.rs
use std::{fs::File, io::BufReader};

use jwt_simple::prelude::*;
use serde::{Deserialize, Serialize};

// a custom payload with serialization must be made
#[derive(Serialize, Deserialize)]
pub struct CustomClaim {
  email: String,
}
pub fn create_jwt(email: String) -> String {
  let f = File::open("key").expect("error reading key file");
  let _reader = BufReader::new(f);
  let buffer = Vec::new();
  let key = HS256Key::from_bytes(&buffer);
  let customclaim = CustomClaim { email: email };
  // duration of the time token will be valid for
  let time = Duration::from_hours(1u64);
  let claim = Claims::with_custom_claims(customclaim, time);
  let token = key.authenticate(claim).expect("fail to create token");
  token
}
