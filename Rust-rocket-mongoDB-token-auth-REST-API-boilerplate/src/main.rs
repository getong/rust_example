#[macro_use]
extern crate rocket;

use rocket::{
  http::{Method, Status},
  serde::json::Json,
};
use rocket_cors::{AllowedOrigins, CorsOptions};

use crate::{
  constants::{UNAUTHORIZED, UNKNOWN},
  database::connect_to_db::init,
  error_response::error_responses::{
    ErrorResponse, NOT_FOUND_JSON, UNAUTHORIZED_JSON, UNKNOWN_JSON,
  },
  helper::check_valid_text,
  routes::{
    authorization::{login::login, registration::registration},
    routes::{
      delete_user::delete_user,
      get_data_user::get_data_user,
      hello_name::{hello_name_user, hello_world},
      patch_user::edit_user,
      refresh_tokens::refresh_tokens,
    },
  },
};

pub mod constants;
mod database;
pub mod error_response;
mod helper;
mod models;
mod private;
mod routes;

#[launch]
async fn rocket() -> _ {
  let cors = CorsOptions::default()
    .allowed_origins(AllowedOrigins::all())
    .allowed_methods(
      vec![Method::Get, Method::Post, Method::Patch, Method::Delete]
        .into_iter()
        .map(From::from)
        .collect(),
    )
    .allow_credentials(true);
  rocket::build()
    .attach(init().await)
    .mount(
      "/api/v1",
      routes![
        registration,
        login,
        hello_name_user,
        hello_world,
        refresh_tokens,
        delete_user,
        edit_user,
        get_data_user
      ],
    )
    .manage(cors.to_cors())
    .register(
      "/",
      catchers![unauthorized, not_found, internal_sever_error,],
    )
}

#[catch(401)]
pub fn unauthorized() -> Json<ErrorResponse> {
  Json(UNAUTHORIZED_JSON)
}

#[catch(404)]
pub fn not_found() -> Json<ErrorResponse> {
  Json(NOT_FOUND_JSON)
}

#[catch(500)]
pub fn internal_sever_error() -> Json<ErrorResponse> {
  Json(UNKNOWN_JSON)
}
