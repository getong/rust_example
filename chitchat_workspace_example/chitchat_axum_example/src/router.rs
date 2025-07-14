use aide::{
  axum::{ApiRouter, IntoApiResponse, routing::get},
  openapi::OpenApi,
  scalar::Scalar,
};
use axum::{Extension, Json};

use crate::api::{AppState, get_members_docs, get_state_docs, update_service_docs};

pub fn create_router() -> ApiRouter<AppState> {
  ApiRouter::new()
    .api_route("/", get_state_docs().into())
    .api_route("/members", get_members_docs().into())
    .api_route("/update_service", update_service_docs().into())
    .route("/docs", Scalar::new("/docs/private/api.json").axum_route())
    .route("/docs/private/api.json", get(serve_docs))
}

async fn serve_docs(Extension(api): Extension<OpenApi>) -> impl IntoApiResponse {
  Json(api)
}
