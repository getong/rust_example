use crate::models::request::login_request::LoginRequest;
use crate::models::request::registration_request::RegistrationRequest;
use rocket::serde::json::Json;

pub mod login;
pub mod registration;
pub mod token;

pub enum RegistrationRequestError {
    Ok(Json<RegistrationRequest>),
    NoneRegistrationRequest,
    BadFirstName,
    BadLastName,
    BadLogin,
    BadPassword,
    BadMail,
}

pub enum LoginRequestError {
    Ok(Json<LoginRequest>),
    NoneLoginRequest,
    BadLogin,
    BadPassword,
}
