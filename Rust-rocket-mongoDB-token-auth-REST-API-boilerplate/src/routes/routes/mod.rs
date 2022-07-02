use crate::models::request::patch_request::EditUserRequest;
use rocket::serde::json::Json;

pub mod delete_user;
pub mod get_data_user;
pub mod hello_name;
pub mod patch_user;
pub mod refresh_tokens;

enum HelloNameError {
    OnlyLogin(String),
    NoOnlyLogin(String),
    ErrorID,
}

pub enum EditUserRequestError {
    Ok(Json<EditUserRequest>),
    NoneEditModel,
    BadMail,
    BadLogin,
    BadFirstName,
    BadLastName,
}
