use axum::{
  body::Body,
  extract::{Json, Request},
  http,
  http::{Response, StatusCode},
  middleware::Next,
  response::IntoResponse,
};
use bcrypt::verify;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, TokenData, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize)]
pub struct Cliams {
  pub exp: usize,
  pub iat: usize,
  pub email: String,
}

pub struct AuthError {
  message: String,
  status_code: StatusCode,
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, bcrypt::BcryptError> {
  verify(password, hash)
}

// pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
//   let hash = hash(password, DEFAULT_COST)?;
//   Ok(hash)
// }

impl IntoResponse for AuthError {
  fn into_response(self) -> Response<Body> {
    let body = Json(json!({
        "error": self.message,
    }));

    (self.status_code, body).into_response()
  }
}

pub fn encode_jwt(email: String) -> Result<String, StatusCode> {
  let jwt_token: String = "randomstring".to_string();

  let now = Utc::now();
  let expire: chrono::TimeDelta = Duration::hours(24);
  let exp: usize = (now + expire).timestamp() as usize;
  let iat: usize = now.timestamp() as usize;

  let claim = Cliams { iat, exp, email };
  let secret = jwt_token.clone();

  encode(
    &Header::default(),
    &claim,
    &EncodingKey::from_secret(secret.as_ref()),
  )
  .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub fn decode_jwt(jwt: String) -> Result<TokenData<Cliams>, StatusCode> {
  let secret = "randomstring".to_string();

  let result: Result<TokenData<Cliams>, StatusCode> = decode(
    &jwt,
    &DecodingKey::from_secret(secret.as_ref()),
    &Validation::default(),
  )
  .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
  result
}

#[derive(Clone)]
pub struct CurrentUser {
  pub email: String,
  pub first_name: String,
  pub last_name: String,
  pub password_hash: String,
}

pub async fn authorize(mut req: Request, next: Next) -> Result<Response<Body>, AuthError> {
  let auth_header = req.headers_mut().get(http::header::AUTHORIZATION);

  let auth_header = match auth_header {
    Some(header) => header.to_str().map_err(|_| AuthError {
      message: "Empty header is not allowed".to_string(),
      status_code: StatusCode::FORBIDDEN,
    })?,
    None => {
      return Err(AuthError {
        message: "Please add the JWT token to the header".to_string(),
        status_code: StatusCode::FORBIDDEN,
      })
    }
  };

  let mut header = auth_header.split_whitespace();

  let (_bearer, token) = (header.next(), header.next());

  let token_data = match decode_jwt(token.unwrap().to_string()) {
    Ok(data) => data,
    Err(_) => {
      return Err(AuthError {
        message: "Unable to decode token".to_string(),
        status_code: StatusCode::UNAUTHORIZED,
      })
    }
  };

  // Fetch the user details from the database
  let current_user = match retrieve_user_by_email(&token_data.claims.email) {
    Some(user) => user,
    None => {
      return Err(AuthError {
        message: "You are not an authorized user".to_string(),
        status_code: StatusCode::UNAUTHORIZED,
      })
    }
  };

  req.extensions_mut().insert(current_user);
  Ok(next.run(req).await)
}

#[derive(Deserialize)]
pub struct SignInData {
  pub email: String,
  pub password: String,
}

pub async fn sign_in(Json(user_data): Json<SignInData>) -> Result<Json<String>, StatusCode> {
  // 1. Retrieve user from the database
  let user = match retrieve_user_by_email(&user_data.email) {
    Some(user) => user,
    None => return Err(StatusCode::UNAUTHORIZED), // User not found
  };

  // 2. Compare the password
  if !verify_password(&user_data.password, &user.password_hash)
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
  // Handle bcrypt errors
  {
    return Err(StatusCode::UNAUTHORIZED); // Wrong password
  }

  // 3. Generate JWT
  let token = encode_jwt(user.email).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

  // 4. Return the token
  Ok(Json(token))
}

fn retrieve_user_by_email(_email: &str) -> Option<CurrentUser> {
  let current_user: CurrentUser = CurrentUser {
    email: "myemail@gmail.com".to_string(),
    first_name: "Eze".to_string(),
    last_name: "Sunday".to_string(),
    password_hash: "$2b$12$Gwf0uvxH3L7JLfo0CC/NCOoijK2vQ/wbgP.LeNup8vj6gg31IiFkm".to_string(),
  };
  Some(current_user)
}
