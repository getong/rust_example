use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;

use crate::constants::{
    ALREADY_REGISTERED_LOGIN, ALREADY_REGISTERED_MAIL, LEN_FIRST_NAME, LEN_LAST_NAME, LEN_LOGIN,
    LEN_PASSWORD, UNKNOWN, WEAK_LOGIN, WEAK_PASSWORD, WRONG_FIRST_NAME, WRONG_LAST_NAME,
    WRONG_MAIL, WRONG_REQUEST,
};
use crate::database::connect_to_db::MongoDB;
use crate::database::RegistrationError;
use crate::error_response::error_responses::ErrorResponse;
use crate::models::request::registration_request::RegistrationRequest;
use crate::models::tokens::Token;
use crate::routes::authorization::RegistrationRequestError;
use crate::routes::validator_authorization::valid_registration_data_user;
use crate::routes::TypeValidDataFromRegistration;

#[post(
    "/registration",
    format = "json",
    data = "<option_registration_request>"
)]
pub async fn registration(
    database: &State<MongoDB>,
    option_registration_request: Option<Json<RegistrationRequest>>,
) -> Result<Json<Token>, (Status, Json<ErrorResponse>)> {
    match check_registration_request(option_registration_request) {
        RegistrationRequestError::Ok(registration_request) => {
            match database.registration(registration_request).await {
                Ok(RegistrationError::Ok(token)) => Ok(Json(Token {
                    token: token.token,
                    refresh_token: token.refresh_token,
                })),
                Ok(RegistrationError::AlreadyRegisteredByEmail) => Err(ALREADY_REGISTERED_MAIL),
                Ok(RegistrationError::AlreadyRegisteredByLogin) => Err(ALREADY_REGISTERED_LOGIN),
                Ok(RegistrationError::WrongPassword) => Err(WEAK_PASSWORD),
                Ok(RegistrationError::Unknown) => Err(UNKNOWN),
                Err(_) => Err(UNKNOWN),
            }
        }
        RegistrationRequestError::NoneRegistrationRequest => Err(WRONG_REQUEST),
        RegistrationRequestError::BadFirstName => Err(WRONG_FIRST_NAME),
        RegistrationRequestError::BadLastName => Err(WRONG_LAST_NAME),
        RegistrationRequestError::BadLogin => Err(WEAK_LOGIN),
        RegistrationRequestError::BadPassword => Err(WEAK_PASSWORD),
        RegistrationRequestError::BadMail => Err(WRONG_MAIL),
    }
}

fn check_registration_request(
    option_registration_request: Option<Json<RegistrationRequest>>,
) -> RegistrationRequestError {
    match option_registration_request {
        None => RegistrationRequestError::NoneRegistrationRequest,
        Some(registration_request) => {
            match valid_registration_data_user(
                &registration_request,
                LEN_FIRST_NAME,
                LEN_LAST_NAME,
                LEN_LOGIN,
                LEN_PASSWORD,
            ) {
                TypeValidDataFromRegistration::Ok => {
                    RegistrationRequestError::Ok(registration_request)
                }
                TypeValidDataFromRegistration::BadFirstName => {
                    RegistrationRequestError::BadFirstName
                }
                TypeValidDataFromRegistration::BadLastName => RegistrationRequestError::BadLastName,
                TypeValidDataFromRegistration::BadLogin => RegistrationRequestError::BadLogin,
                TypeValidDataFromRegistration::BadPassword => RegistrationRequestError::BadPassword,
                TypeValidDataFromRegistration::BadMail => RegistrationRequestError::BadMail,
            }
        }
    }
}
