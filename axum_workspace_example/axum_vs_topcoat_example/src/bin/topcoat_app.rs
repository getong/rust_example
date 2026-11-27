//! 用 topcoat 实现的最小服务端渲染页面。
//!
//! 运行: cargo run -p axum_vs_topcoat_example --bin topcoat_app
//! 然后访问 http://127.0.0.1:3000
//! 监听地址可用 HOST / PORT 环境变量覆盖（默认 127.0.0.1:3000）。

use topcoat::{
  Result,
  context::Cx,
  router::{Router, RouterBuilderDiscoverExt, page},
  view::{View, component, view},
};

#[tokio::main]
async fn main() {
  topcoat::start(Router::builder().discover().build())
    .await
    .unwrap();
}

#[page("/")]
async fn home(cx: &Cx) -> Result<impl View> {
  Ok(view! {
    <!DOCTYPE html>
    <html>
      <head>
        <meta charset="utf-8">
        <title>"Axum vs Topcoat"</title>
      </head>
      <body>
        hello(name: "World")
        <p>"This page is rendered by " <strong>"topcoat"</strong> "."</p>
        <p>"HTML is written with the type-checked " <code>"view!"</code> " macro."</p>
      </body>
    </html>
  })
}

#[component]
async fn hello(cx: &Cx, name: &str) -> Result<impl View> {
  Ok(view! { <h1>"Hello, " (name) "!"</h1> })
}
