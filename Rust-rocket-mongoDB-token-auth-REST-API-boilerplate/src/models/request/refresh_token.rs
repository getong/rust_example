use serde::Deserialize;

#[derive(Deserialize)]
pub struct RefreshToken {
    pub(crate) refresh_token: String,
}
