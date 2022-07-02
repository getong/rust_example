use crate::error_response::error_responses::{
    ErrorResponse, ALREADY_REGISTERED_EMAIL_JSON, ALREADY_REGISTERED_LOGIN_JSON,
    ERROR_ALREADY_REGISTERED_STATUS, ERROR_NOT_FOUND_STATUS, ERROR_UNAUTHORIZED_STATUS,
    ERROR_UNKNOWN_STATUS, ERROR_WEAK_LOGIN_STATUS, ERROR_WEAK_PASSWORD_STATUS,
    ERROR_WRONG_FIRST_NAME_STATUS, ERROR_WRONG_LAST_NAME_STATUS, ERROR_WRONG_MAIL_STATUS,
    ERROR_WRONG_REQUEST_STATUS, NOT_FOUND_JSON, UNAUTHORIZED_JSON, UNKNOWN_JSON, WEAK_LOGIN_JSON,
    WEAK_PASSWORD_JSON, WRONG_FIRST_NAME_JSON, WRONG_LAST_NAME_JSON, WRONG_MAIL_JSON,
    WRONG_REQUEST_JSON,
};
use rocket::http::Status;
use rocket::serde::json::Json;

pub const EXPIRATION_REFRESH_TOKEN: i64 = 3600 * 24 * 30;
pub const EXPIRATION_TOKEN: i64 = 3600;

pub struct LenText {
    pub(crate) min: usize,
    pub(crate) max: usize,
}

//min && max len login
pub const LEN_LOGIN: LenText = LenText { min: 2, max: 200 };

//min && max len password
pub const LEN_PASSWORD: LenText = LenText { min: 8, max: 200 };

//min && max len first name
pub const LEN_FIRST_NAME: LenText = LenText { min: 2, max: 100 };

//min && max len last name
pub const LEN_LAST_NAME: LenText = LenText { min: 2, max: 150 };

//errors
pub const WRONG_REQUEST: (Status, Json<ErrorResponse>) =
    (ERROR_WRONG_REQUEST_STATUS, Json(WRONG_REQUEST_JSON));

pub const WRONG_MAIL: (Status, Json<ErrorResponse>) =
    (ERROR_WRONG_MAIL_STATUS, Json(WRONG_MAIL_JSON));

pub const ALREADY_REGISTERED_LOGIN: (Status, Json<ErrorResponse>) = (
    ERROR_ALREADY_REGISTERED_STATUS,
    Json(ALREADY_REGISTERED_LOGIN_JSON),
);

pub const ALREADY_REGISTERED_MAIL: (Status, Json<ErrorResponse>) = (
    ERROR_ALREADY_REGISTERED_STATUS,
    Json(ALREADY_REGISTERED_EMAIL_JSON),
);

pub const WEAK_PASSWORD: (Status, Json<ErrorResponse>) =
    (ERROR_WEAK_PASSWORD_STATUS, Json(WEAK_PASSWORD_JSON));

pub const WEAK_LOGIN: (Status, Json<ErrorResponse>) =
    (ERROR_WEAK_LOGIN_STATUS, Json(WEAK_LOGIN_JSON));

pub const UNKNOWN: (Status, Json<ErrorResponse>) = (ERROR_UNKNOWN_STATUS, Json(UNKNOWN_JSON));

pub const UNAUTHORIZED: (Status, Json<ErrorResponse>) =
    (ERROR_UNAUTHORIZED_STATUS, Json(UNAUTHORIZED_JSON));

pub const WRONG_FIRST_NAME: (Status, Json<ErrorResponse>) =
    (ERROR_WRONG_FIRST_NAME_STATUS, Json(WRONG_FIRST_NAME_JSON));

pub const WRONG_LAST_NAME: (Status, Json<ErrorResponse>) =
    (ERROR_WRONG_LAST_NAME_STATUS, Json(WRONG_LAST_NAME_JSON));

pub const NOT_FOUND: (Status, Json<ErrorResponse>) = (ERROR_NOT_FOUND_STATUS, Json(NOT_FOUND_JSON));
