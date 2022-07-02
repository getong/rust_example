use crate::check_valid_text;
use crate::constants::LenText;
use crate::helper::check_valid_name;
use crate::models::request::patch_request::EditUserRequest;
use crate::models::request::registration_request::RegistrationRequest;
use crate::routes::TypeValidDataFromRegistration::*;
use crate::routes::{TypeValidDataFromRegistration, TypeValidMail, TypeValidTwoStr};
use regex::Regex;
use rocket::serde::json::Json;

pub fn get_valid_login_and_password(
    login: &str,
    password: &str,
    max_min_len_login: LenText,
    max_min_len_password: LenText,
) -> TypeValidTwoStr {
    if check_valid_text(login, max_min_len_login.max, max_min_len_login.min) {
        if check_valid_text(password, max_min_len_password.max, max_min_len_password.min) {
            TypeValidTwoStr::Ok
        } else {
            TypeValidTwoStr::BadSecond
        }
    } else {
        TypeValidTwoStr::BadFirst
    }
}

pub fn valid_registration_data_user(
    registration_request: &Json<RegistrationRequest>,
    max_min_len_first_name: LenText,
    max_min_len_last_name: LenText,
    max_min_len_login: LenText,
    max_min_len_password: LenText,
) -> TypeValidDataFromRegistration {
    match get_valid_first_and_last_names(
        &registration_request.first_name,
        &registration_request.last_name,
        max_min_len_first_name,
        max_min_len_last_name,
    ) {
        TypeValidTwoStr::Ok => {
            match get_valid_login_and_password(
                &registration_request.login,
                &registration_request.password,
                max_min_len_login,
                max_min_len_password,
            ) {
                TypeValidTwoStr::Ok => match get_valid_mail(&registration_request.mail) {
                    TypeValidMail::Ok => Ok,
                    TypeValidMail::BadMail => BadMail,
                },
                TypeValidTwoStr::BadFirst => BadLogin,
                TypeValidTwoStr::BadSecond => BadPassword,
            }
        }
        TypeValidTwoStr::BadFirst => BadFirstName,
        TypeValidTwoStr::BadSecond => BadLastName,
    }
}

pub enum ValidEditModelError {
    Ok,
    BadMail,
    BadLogin,
    BadFirstName,
    BadLastName,
}

pub fn valid_edit_model(
    edit_model: &Json<EditUserRequest>,
    max_min_len_first_name: LenText,
    max_min_len_last_name: LenText,
    max_min_len_login: LenText,
) -> ValidEditModelError {
    match get_valid_first_and_last_names(
        &edit_model.first_name,
        &edit_model.last_name,
        max_min_len_first_name,
        max_min_len_last_name,
    ) {
        TypeValidTwoStr::Ok => {
            match check_valid_text(
                &edit_model.login,
                max_min_len_login.max,
                max_min_len_login.min,
            ) {
                true => match get_valid_mail(&edit_model.mail) {
                    TypeValidMail::Ok => ValidEditModelError::Ok,
                    TypeValidMail::BadMail => ValidEditModelError::BadMail,
                },
                false => ValidEditModelError::BadLogin,
            }
        }
        TypeValidTwoStr::BadFirst => ValidEditModelError::BadFirstName,
        TypeValidTwoStr::BadSecond => ValidEditModelError::BadLastName,
    }
}

pub fn get_valid_first_and_last_names(
    first_str: &str,
    second_str: &str,
    max_min_len_first_name: LenText,
    max_min_len_last_name: LenText,
) -> TypeValidTwoStr {
    if check_valid_name(
        first_str,
        max_min_len_first_name.max,
        max_min_len_first_name.min,
    ) {
        if check_valid_name(
            second_str,
            max_min_len_last_name.max,
            max_min_len_last_name.min,
        ) {
            TypeValidTwoStr::Ok
        } else {
            TypeValidTwoStr::BadSecond
        }
    } else {
        TypeValidTwoStr::BadFirst
    }
}

pub fn get_valid_mail(mail: &str) -> TypeValidMail {
    match Regex::new(r"^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([a-z0-9]+)*\.[a-z]{2,6})")
    {
        Result::Ok(regex) => match regex.is_match(mail) {
            true => TypeValidMail::Ok,
            false => TypeValidMail::BadMail,
        },
        Err(_) => TypeValidMail::BadMail,
    }
}
