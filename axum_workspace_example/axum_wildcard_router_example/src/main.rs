use axum::{
  extract::Path,
  response::IntoResponse,
  routing::{Router, get},
};

async fn single_param(Path(param): Path<String>) -> impl IntoResponse {
  format!("Single parameter: {}", param)
}

async fn wildcard_param(Path(params): Path<Vec<String>>) -> impl IntoResponse {
  format!("Wildcard parameters: {:?}", params)
}

#[tokio::main]
async fn main() {
  let app = Router::new()
    .route("/single/{param}", get(single_param))
    .route("/wildcard/{*params}", get(wildcard_param));

  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, app).await.unwrap();
}

// curl http://127.0.0.1:3000/single/example
// curl http://127.0.0.1:3000/wildcard/one/two/three
