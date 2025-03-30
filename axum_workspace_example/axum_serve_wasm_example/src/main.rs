use std::fs;

use axum::{
  Router,
  body::Body,
  response::{IntoResponse, Response},
  routing::get,
};
use http::{HeaderValue, header::CONTENT_TYPE};
// use tower_http::services::ServeDir;

#[tokio::main]
async fn main() {
  let app = Router::new()
    .route("/", get(serve_index))
    .route("/static/wasm_project.js", get(serve_js))
    .route("/static/wasm_project_bg.wasm", get(serve_wasm));
    // .nest_service("/static", ServeDir::new("./static"));

  let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
    .await
    .unwrap();

  println!("listening on http://127.0.0.1:3000");
  axum::serve(listener, app).await.unwrap();
}

async fn serve_index() -> impl IntoResponse {
  let index_html = fs::read_to_string("static/index.html").unwrap();
  Response::builder()
    .header(CONTENT_TYPE, HeaderValue::from_static("text/html"))
    .body(Body::from(index_html))
    .unwrap()
}

async fn serve_js() -> impl IntoResponse {
  let js = fs::read("static/wasm_project.js").unwrap();
  Response::builder()
    .header(
      CONTENT_TYPE,
      HeaderValue::from_static("application/javascript"),
    )
    .body(Body::from(js))
    .unwrap()
}

async fn serve_wasm() -> impl IntoResponse {
  let wasm_bytes = fs::read("static/wasm_project_bg.wasm").unwrap();
  Response::builder()
    .header(CONTENT_TYPE, HeaderValue::from_static("application/wasm"))
    .body(Body::from(wasm_bytes))
    .unwrap()
}
