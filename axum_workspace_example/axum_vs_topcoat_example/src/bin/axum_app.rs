//! 用 axum 实现的最小服务端渲染页面。
//!
//! 运行: cargo run -p axum_vs_topcoat_example --bin axum_app
//! 然后访问 http://127.0.0.1:3000

use axum::{Router, response::Html, routing::get};

#[tokio::main]
async fn main() {
  let app = Router::new().route("/", get(home));

  let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
    .await
    .unwrap();
  println!("axum app listening on http://127.0.0.1:3000");
  axum::serve(listener, app).await.unwrap();
}

async fn home() -> Html<&'static str> {
  Html(PAGE)
}

const PAGE: &str = r#"<!DOCTYPE html>
<html>
  <head>
    <meta charset="utf-8">
    <title>Axum vs Topcoat</title>
  </head>
  <body>
    <h1>Hello, world!</h1>
    <p>This page is rendered by <strong>axum</strong>.</p>
    <p>HTML is written by hand (or by a template engine you add yourself).</p>
  </body>
</html>
"#;
