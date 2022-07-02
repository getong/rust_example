pub mod connect_to_db;
pub mod impl_mondo_db;

use crate::models::tokens::Token;

pub enum LoginError {
    Ok(Token),
    WrongLogin,
    WrongPassword,
    Unknown,
}

pub enum RegistrationError {
    Ok(Token),
    AlreadyRegisteredByEmail,
    AlreadyRegisteredByLogin,
    WrongPassword,
    Unknown,
}

pub enum FindUserBy {
    UserNotFound,
    UserFoundByLogin,
    UserFoundByEmail,
}
