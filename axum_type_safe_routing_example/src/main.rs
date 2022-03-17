use axum::{http::StatusCode, response::IntoResponse, Json, Router};
use axum_extra::routing::{RouterExt, TypedPath};
use serde::{Deserialize, Serialize};

fn app() -> Router {
    Router::new().typed_get(user_detail)
}

#[derive(TypedPath, Deserialize, Serialize)]
#[typed_path("/api/users/:user_id")]
pub struct PathParam {
    pub user_id: String,
}

pub async fn user_detail(params: PathParam) -> impl IntoResponse {
    (StatusCode::OK, Json(params)).into_response()
}

#[tokio::main]
async fn main() {
    let app = app();

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
