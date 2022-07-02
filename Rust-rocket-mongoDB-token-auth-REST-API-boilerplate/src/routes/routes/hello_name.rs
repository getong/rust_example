use crate::database::connect_to_db::MongoDB;
use crate::helper::{parse_id_and_find_user_by_id, FindUserById};
use crate::models::hello_response::HelloNameResponse;
use crate::routes::routes::HelloNameError;
use crate::{ErrorResponse, Status, UNAUTHORIZED};

use crate::routes::authorization::token::request_access_token::AuthorizedUser;
use rocket::serde::json::Json;
use rocket::State;

//(private) request with authorization model (token)
#[get("/private/hello")]
pub async fn hello_name_user(
    auth: AuthorizedUser,
    database: &State<MongoDB>,
) -> Result<Json<HelloNameResponse>, (Status, Json<ErrorResponse>)> {
    match check_from_db_real_names(database, auth.user_id).await {
        HelloNameError::OnlyLogin(res_only_login) => Ok(Json(HelloNameResponse {
            greetings: res_only_login,
        })),
        HelloNameError::NoOnlyLogin(res_no_only_login) => Ok(Json(HelloNameResponse {
            greetings: res_no_only_login,
        })),
        HelloNameError::ErrorID => Err(UNAUTHORIZED),
    }
}

//we check if the first and last names are in the database
async fn check_from_db_real_names(database: &State<MongoDB>, id_str: String) -> HelloNameError {
    match parse_id_and_find_user_by_id(database, id_str).await {
        FindUserById::Ok(user) => {
            if user.first_name == "" || user.last_name == "" {
                HelloNameError::OnlyLogin(format!("Hello {}", user.login,))
            } else {
                HelloNameError::NoOnlyLogin(format!(
                    "Hello {} <{}> {}",
                    user.first_name, user.login, user.last_name
                ))
            }
        }
        FindUserById::NoneUser => HelloNameError::ErrorID,
        FindUserById::BadId => HelloNameError::ErrorID,
    }
}

//(public) hello world
#[get("/public/hello")]
pub async fn hello_world() -> Json<&'static str> {
    Json("Hello world")
}
