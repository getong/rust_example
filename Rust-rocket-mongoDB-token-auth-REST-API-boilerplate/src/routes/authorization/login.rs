use rocket::{http::Status, serde::json::Json, State};

use crate::{
  constants::{LEN_LOGIN, LEN_PASSWORD, WRONG_REQUEST},
  database::{connect_to_db::MongoDB, LoginError},
  error_response::error_responses::ErrorResponse,
  models::{request::login_request::LoginRequest, tokens::Token},
  routes::{
    authorization::LoginRequestError, validator_authorization::get_valid_login_and_password,
    TypeValidTwoStr,
  },
};

#[post("/login", format = "json", data = "<option_login_request>")]
pub async fn login(
  database: &State<MongoDB>,
  option_login_request: Option<Json<LoginRequest>>,
) -> Result<Json<Token>, (Status, Json<ErrorResponse>)> {
  match check_login_request(option_login_request) {
    LoginRequestError::Ok(login_request) => match login_match(database, login_request).await {
      Ok(tokens) => Ok(Json(tokens)),
      Err(_) => Err(WRONG_REQUEST),
    },
    LoginRequestError::NoneLoginRequest => Err(WRONG_REQUEST),
    LoginRequestError::BadLogin => Err(WRONG_REQUEST),
    LoginRequestError::BadPassword => Err(WRONG_REQUEST),
  }
}

fn check_login_request(option_login_request: Option<Json<LoginRequest>>) -> LoginRequestError {
  match option_login_request {
    None => LoginRequestError::NoneLoginRequest,
    Some(login_request) => {
      match get_valid_login_and_password(
        &login_request.login,
        &login_request.password,
        LEN_LOGIN,
        LEN_PASSWORD,
      ) {
        TypeValidTwoStr::Ok => LoginRequestError::Ok(login_request),
        TypeValidTwoStr::BadFirst => LoginRequestError::BadLogin,
        TypeValidTwoStr::BadSecond => LoginRequestError::BadPassword,
      }
    }
  }
}

async fn login_match(
  database: &State<MongoDB>,
  login_request: Json<LoginRequest>,
) -> Result<Token, ()> {
  match database.login(login_request).await {
    Ok(LoginError::Ok(tokens)) => Ok(tokens),
    Ok(LoginError::WrongPassword) => Err(()),
    Ok(LoginError::WrongLogin) => Err(()),
    Ok(LoginError::Unknown) => Err(()),
    Err(_) => Err(()),
  }
}
