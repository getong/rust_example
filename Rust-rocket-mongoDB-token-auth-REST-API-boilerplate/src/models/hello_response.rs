use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct HelloNameResponse {
    pub(crate) greetings: String,
}
