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
  // Example JWT token (this is a sample token for demonstration)
  // In a real application, this would come from a request header
  let token = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.\
               eyJzdWIiOiIxMjM0NTY3ODkwIiwiY29tcGFueSI6IkFjbWUgQ29ycCIsImV4cCI6MTczNTg1NDAwMH0.\
               rJVXyqXDwUUYbkF8Z8X1nF5NvUVQd5q9mVzY0W7X1A8";

  // Secret key used to verify the token (should be kept secure)
  let secret = "your-256-bit-secret";

  println!("JWT Encoding and Decoding Examples\n");

  // Example 0: Encode a new JWT token
  encode_jwt_example(secret);

  // Example 1: Decode header without verification
  decode_header_example(token);

  // Example 2: Decode and verify token
  decode_and_verify_example(token, secret);

  // Example 3: Handle expired token
  handle_expired_token_example();

  // Example 4: Decode with custom validation
  decode_with_custom_validation_example(token, secret);
}

fn encode_jwt_example(secret: &str) {
  println!("=== Example 0: Encoding a JWT ===");

  // Create claims
  let claims = Claims {
    sub: "1234567890".to_string(),
    company: "Example Corp".to_string(),
    exp: (std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap()
      .as_secs()
      + 3600) as usize, // Expires in 1 hour
  };

  // Create encoding key and header
  let encoding_key = EncodingKey::from_secret(secret.as_ref());
  let header = Header::new(Algorithm::HS256);

  match encode(&header, &claims, &encoding_key) {
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

  // This is an expired token for demonstration
  let expired_token =
    "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.\
     eyJzdWIiOiIxMjM0NTY3ODkwIiwiY29tcGFueSI6IkFjbWUgQ29ycCIsImV4cCI6MTYwOTQ1OTIwMH0.123456789";
  let secret = "your-256-bit-secret";

  let decoding_key = DecodingKey::from_secret(secret.as_ref());
  let validation = Validation::new(Algorithm::HS256);

  match decode::<Claims>(expired_token, &decoding_key, &validation) {
    Ok(_) => {
      println!("Token is valid (unexpected)");
    }
    Err(err) => {
      println!("Token validation failed: {}", err);
      if let jsonwebtoken::errors::ErrorKind::ExpiredSignature = err.kind() {
        println!("  -> This is expected for an expired token");
      }
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
}
