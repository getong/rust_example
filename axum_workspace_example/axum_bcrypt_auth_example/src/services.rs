use axum::{response::IntoResponse, Extension, Json};
use serde::{Deserialize, Serialize};

use crate::auth::CurrentUser;

#[derive(Serialize, Deserialize)]
struct UserResponse {
  email: String,
  first_name: String,
  last_name: String,
}

pub async fn hello(Extension(current_user): Extension<CurrentUser>) -> impl IntoResponse {
  Json(UserResponse {
    email: current_user.email,
    first_name: current_user.first_name,
    last_name: current_user.last_name,
  })
}
