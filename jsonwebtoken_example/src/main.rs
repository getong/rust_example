use aws_lc_rs::{
  encoding::{AsDer, PublicKeyX509Der},
  signature::{self, Ed25519KeyPair, KeyPair},
};
use jsonwebtoken::{
  Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, decode_header, encode,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
  sub: String,
  company: String,
  exp: usize,
}

fn main() {
  // Secret key used to verify the token (should be kept secure)
  let secret = "your-256-bit-secret";
  let demo_claims = Claims {
    sub: "1234567890".to_string(),
    company: "Example Corp".to_string(),
    exp: one_hour_from_now(),
  };
  let token = build_hs256_token(&demo_claims, secret).expect("failed to build demo HS256 token");

  println!("JWT Encoding and Decoding Examples\n");

  // Example 0: Encode a new JWT token
  encode_jwt_example(secret);

  // Example 1: Decode header without verification
  decode_header_example(&token);

  // Example 2: Decode and verify token
  decode_and_verify_example(&token, secret);

  // Example 3: Handle expired token
  handle_expired_token_example();

  // Example 4: Decode with custom validation
  decode_with_custom_validation_example(&token, secret);

  // Example 5: Generate Ed25519 keys with aws-lc-rs and use them with jsonwebtoken
  aws_lc_rs_eddsa_example();
}

fn encode_jwt_example(secret: &str) {
  println!("=== Example 0: Encoding a JWT ===");

  // Create claims
  let claims = Claims {
    sub: "1234567890".to_string(),
    company: "Example Corp".to_string(),
    exp: one_hour_from_now(), // Expires in 1 hour
  };

  match build_hs256_token(&claims, secret) {
    Ok(token) => {
      println!("JWT encoded successfully:");
      println!("  Token: {}", token);
      println!("  Claims encoded:");
      println!("    Subject: {}", claims.sub);
      println!("    Company: {}", claims.company);
      println!("    Expires at: {}", claims.exp);

      // Immediately decode it to verify
      println!("\n  Verifying the encoded token:");
      let decoding_key = DecodingKey::from_secret(secret.as_ref());
      let validation = Validation::new(Algorithm::HS256);

      match decode::<Claims>(&token, &decoding_key, &validation) {
        Ok(decoded) => {
          println!("    ✓ Token verified successfully");
          println!("    ✓ Decoded subject: {}", decoded.claims.sub);
        }
        Err(err) => {
          println!("    ✗ Token verification failed: {}", err);
        }
      }
    }
    Err(err) => {
      println!("Failed to encode JWT: {}", err);
    }
  }
  println!();
}

fn decode_header_example(token: &str) {
  println!("=== Example 1: Decoding JWT Header ===");

  match decode_header(token) {
    Ok(header) => {
      println!("Header decoded successfully:");
      println!("  Algorithm: {:?}", header.alg);
      println!("  Type: {:?}", header.typ);
      println!("  Key ID: {:?}", header.kid);
    }
    Err(err) => {
      println!("Failed to decode header: {}", err);
    }
  }
  println!();
}

fn decode_and_verify_example(token: &str, secret: &str) {
  println!("=== Example 2: Decoding and Verifying JWT ===");

  let decoding_key = DecodingKey::from_secret(secret.as_ref());
  let validation = Validation::new(Algorithm::HS256);

  match decode::<Claims>(token, &decoding_key, &validation) {
    Ok(token_data) => {
      println!("Token decoded and verified successfully:");
      println!("  Subject: {}", token_data.claims.sub);
      println!("  Company: {}", token_data.claims.company);
      println!("  Expires at: {}", token_data.claims.exp);
      println!("  Header: {:?}", token_data.header);
    }
    Err(err) => {
      println!("Failed to decode/verify token: {}", err);
      match err.kind() {
        jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
          println!("  Reason: Token has expired");
        }
        jsonwebtoken::errors::ErrorKind::InvalidSignature => {
          println!("  Reason: Invalid signature");
        }
        jsonwebtoken::errors::ErrorKind::InvalidToken => {
          println!("  Reason: Invalid token format");
        }
        _ => {
          println!("  Reason: Other error");
        }
      }
    }
  }
  println!();
}

fn handle_expired_token_example() {
  println!("=== Example 3: Handling Expired Token ===");

  let secret = "your-256-bit-secret";
  let expired_claims = Claims {
    sub: "expired-user".to_string(),
    company: "Example Corp".to_string(),
    exp: (chrono::Utc::now().timestamp() - 3600) as usize,
  };

  match build_hs256_token(&expired_claims, secret) {
    Ok(expired_token) => match decode::<Claims>(
      &expired_token,
      &DecodingKey::from_secret(secret.as_ref()),
      &Validation::new(Algorithm::HS256),
    ) {
      Ok(_) => {
        println!("Token is valid (unexpected)");
      }
      Err(err) => {
        println!("Token validation failed: {}", err);
        if let jsonwebtoken::errors::ErrorKind::ExpiredSignature = err.kind() {
          println!("  -> This is expected for an expired token");
        }
      }
    },
    Err(err) => {
      println!("Failed to build expired JWT: {}", err);
    }
  }
  println!();
}

fn decode_with_custom_validation_example(token: &str, secret: &str) {
  println!("=== Example 4: Custom Validation Rules ===");

  let decoding_key = DecodingKey::from_secret(secret.as_ref());
  let mut validation = Validation::new(Algorithm::HS256);

  // Customize validation rules
  validation.leeway = 60; // Allow 60 seconds leeway for exp/nbf/iat
  validation.validate_exp = true; // Validate expiration (default: true)
  validation.validate_nbf = false; // Don't validate "not before" field
  validation.aud = None; // Don't validate audience
  validation.iss = None; // Don't validate issuer
  validation.sub = Some("1234567890".to_string()); // Validate specific subject

  match decode::<Claims>(token, &decoding_key, &validation) {
    Ok(token_data) => {
      println!("Token decoded with custom validation:");
      println!("  Subject: {}", token_data.claims.sub);
      println!("  Company: {}", token_data.claims.company);
    }
    Err(err) => {
      println!("Custom validation failed: {}", err);
    }
  }
  println!();
}

fn aws_lc_rs_eddsa_example() {
  println!("=== Example 5: aws-lc-rs Ed25519 + jsonwebtoken ===");

  match create_jwt_with_aws_lc_rs() {
    Ok(result) => {
      println!("aws-lc-rs generated a fresh Ed25519 keypair.");
      println!("  Public key length: {} bytes", result.public_key_len);
      println!(
        "  Private key (PKCS#8 DER) length: {} bytes",
        result.private_key_len
      );
      println!(
        "  Public key (X.509 DER) length: {} bytes",
        result.public_key_der_len
      );
      println!("  Raw message signature verified with aws-lc-rs: ✓");
      println!("  JWT signed with EdDSA through jsonwebtoken:");
      println!("    Token: {}", result.token);
      println!("  JWT decoded successfully:");
      println!("    Subject: {}", result.claims.sub);
      println!("    Company: {}", result.claims.company);
      println!("    Expires at: {}", result.claims.exp);
      println!("  How it works:");
      println!("    1. aws-lc-rs generates the Ed25519 keypair");
      println!("    2. jsonwebtoken reuses the DER-encoded keys");
      println!("    3. because Cargo enables jsonwebtoken's aws_lc_rs feature,");
      println!("       the EdDSA signing and verification path uses aws-lc-rs internally");
    }
    Err(err) => {
      println!("aws-lc-rs example failed: {}", err);
    }
  }
  println!();
}

#[derive(Debug)]
struct AwsLcJwtExample {
  token: String,
  claims: Claims,
  private_key_len: usize,
  public_key_len: usize,
  public_key_der_len: usize,
}

fn create_jwt_with_aws_lc_rs() -> Result<AwsLcJwtExample, Box<dyn std::error::Error>> {
  const MESSAGE: &[u8] = b"aws-lc-rs signs this message before jsonwebtoken uses the same key";

  // aws-lc-rs is doing the key generation and raw Ed25519 signing here.
  let key_pair = Ed25519KeyPair::generate()?;
  let signature = key_pair.sign(MESSAGE);
  let public_key = key_pair.public_key();

  let verifier = signature::UnparsedPublicKey::new(&signature::ED25519, public_key.as_ref());
  verifier.verify(MESSAGE, signature.as_ref())?;

  // Export the aws-lc-rs keys in DER so jsonwebtoken can use them for EdDSA.
  let private_key_der = key_pair.to_pkcs8()?;
  let public_key_der = AsDer::<PublicKeyX509Der<'static>>::as_der(public_key)?;

  let claims = Claims {
    sub: "aws-lc-rs-user".to_string(),
    company: "Example Corp".to_string(),
    exp: one_hour_from_now(),
  };

  let token = encode(
    &Header::new(Algorithm::EdDSA),
    &claims,
    &EncodingKey::from_ed_der(private_key_der.as_ref()),
  )?;

  let decoded = decode::<Claims>(
    &token,
    &DecodingKey::from_ed_der(public_key_der.as_ref()),
    &Validation::new(Algorithm::EdDSA),
  )?;

  Ok(AwsLcJwtExample {
    token,
    claims: decoded.claims,
    private_key_len: private_key_der.as_ref().len(),
    public_key_len: public_key.as_ref().len(),
    public_key_der_len: public_key_der.as_ref().len(),
  })
}

fn one_hour_from_now() -> usize {
  (std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs()
    + 3600) as usize
}

fn build_hs256_token(claims: &Claims, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
  let encoding_key = EncodingKey::from_secret(secret.as_ref());
  let header = Header::new(Algorithm::HS256);
  encode(&header, claims, &encoding_key)
}

#[cfg(test)]
mod tests {
  use jsonwebtoken::{EncodingKey, Header, encode};

  use super::*;

  #[test]
  fn test_create_and_decode_jwt() {
    let secret = "test-secret";
    let claims = Claims {
      sub: "test-user".to_string(),
      company: "Test Corp".to_string(),
      exp: (chrono::Utc::now().timestamp() + 3600) as usize, // 1 hour from now
    };

    // Create a token
    let encoding_key = EncodingKey::from_secret(secret.as_ref());
    let header = Header::new(Algorithm::HS256);
    let token = encode(&header, &claims, &encoding_key).unwrap();

    // Decode the token
    let decoding_key = DecodingKey::from_secret(secret.as_ref());
    let validation = Validation::new(Algorithm::HS256);
    let decoded = decode::<Claims>(&token, &decoding_key, &validation).unwrap();

    assert_eq!(decoded.claims.sub, "test-user");
    assert_eq!(decoded.claims.company, "Test Corp");
  }

  #[test]
  fn test_aws_lc_rs_generated_ed25519_jwt() {
    let result = create_jwt_with_aws_lc_rs().unwrap();

    assert_eq!(result.claims.sub, "aws-lc-rs-user");
    assert_eq!(result.claims.company, "Example Corp");
    assert!(result.private_key_len > 0);
    assert_eq!(result.public_key_len, 32);
    assert!(result.public_key_der_len > result.public_key_len);
    assert!(!result.token.is_empty());
  }
}
