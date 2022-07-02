use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RegistrationRequest {
    pub login: String,
    pub password: String,

    pub mail: String,

    pub first_name: String,
    pub last_name: String,
}
