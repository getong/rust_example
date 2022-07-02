use crate::constants::WRONG_REQUEST;
use crate::database::connect_to_db::MongoDB;
use crate::helper::{parse_id_and_find_user_by_id, FindUserById};
use crate::routes::authorization::token::request_access_token::AuthorizedUser;
use crate::{ErrorResponse, UNAUTHORIZED, UNKNOWN};
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

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
