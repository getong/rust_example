use rocket::{http::Status, serde::json::Json, State};

use crate::{
  constants::WRONG_REQUEST,
  database::connect_to_db::MongoDB,
  helper::{parse_id_and_find_user_by_id, FindUserById},
  routes::authorization::token::request_access_token::AuthorizedUser,
  ErrorResponse, UNAUTHORIZED, UNKNOWN,
};

#[delete("/user")]
pub async fn delete_user(
  database: &State<MongoDB>,
  auth: AuthorizedUser,
) -> Result<Status, (Status, Json<ErrorResponse>)> {
  match parse_id_and_find_user_by_id(database, auth.user_id).await {
    FindUserById::Ok(user) => match database.delete_user(&user.login).await {
      Ok(_) => Ok(Status::NoContent),
      Err(_) => Err(UNKNOWN),
    },
    FindUserById::NoneUser => Err(WRONG_REQUEST),
    FindUserById::BadId => Err(UNAUTHORIZED),
  }
}
