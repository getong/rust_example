use axum::{Router, routing::get};

use crate::api::{AppState, get_members, get_state, update_service};

pub fn create_router() -> Router<AppState> {
  Router::new()
    .route("/", get(get_state))
    .route("/members", get(get_members))
    .route("/update_service", get(update_service))
}
