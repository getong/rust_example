use axum::{
  Router,
  // body::Body,
  // extract::Path,
  response::Html, // IntoResponse, Response},
  routing::get,
};
use dioxus::prelude::*;
use tower_http::services::ServeDir;
// use http::{HeaderValue, StatusCode, header};
use web_sys::console;

#[tokio::main]
async fn main() {
  let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
    .await
    .unwrap();

  println!("listening on http://127.0.0.1:3000");

  axum::serve(
    listener,
    Router::new()
      .route("/", get(app_endpoint))
      .route("/about", get(about_endpoint))
      .route("/about_prop", get(about_prop_endpoint))
      // .route(
      //   "/target/dioxus_web_sys_example.js",
      //   get(dioxus_web_sys_example_js),
      // )
      // .route(
      //   "/target/dioxus_web_sys_example_bg.wasm",
      //   get(dioxus_web_sys_example_wasm),
      // )
      .nest_service("/target", ServeDir::new("../target"))
      .into_make_service(),
  )
  .await
  .unwrap();
}

fn app() -> Element {
  rsx! { div { "hello world" } }
}

async fn app_endpoint() -> Html<String> {
  let mut app = VirtualDom::new(app);
  app.rebuild_in_place();
  Html(dioxus_ssr::render(&app))
}

async fn about_endpoint() -> Html<String> {
  let mut app = VirtualDom::new(About);
  app.rebuild_in_place();
  Html(dioxus_ssr::render(&app))
}

#[component]
fn About() -> Element {
  rsx!(
      div {
          h1 { "hello from client!" }
          button {
              onclick: move |_| {
                  console::log_1(&"hello world".into());
              },
              "click me"
          }
      }
  )
}

async fn about_prop_endpoint() -> Html<String> {
  let props = AboutProps {
    message: "hello from client!".to_string(),
  };
  let mut app = VirtualDom::new_with_props(AboutProp, props);
  app.rebuild_in_place();
  Html(dioxus_ssr::render(&app))
}

#[derive(Props, Clone, PartialEq)]
struct AboutProps {
  message: String,
}

#[component]
fn AboutProp(props: AboutProps) -> Element {
  rsx!(
      div {
          h1 { "{props.message}" }
      }
      script {
          r#type: "module",
          dangerous_inner_html: r#"
console.log('hello from script tag');

import start from '../target/dioxus_web_sys_example.js';
start();

"#
      }
  )
}

// #[component]
// fn AboutProp(props: AboutProps) -> Element {
//   rsx!(
//         div {
//             h1 { "{props.message}" }
//         }
//         script {
//             r#type: "module",
//             dangerous_inner_html: r#"
// import init, { say_hello } from '../target/dioxus_web_sys_example.js';
// init().then(() => {
//   say_hello();
// });
// "#
//         }

//   )
// }

// async fn dioxus_web_sys_example_js() -> impl IntoResponse {
//   (
//     [(header::CONTENT_TYPE, "text/javascript")],
//     include_str!("../../target/dioxus_web_sys_example.js"),
//   )
// }

// async fn dioxus_web_sys_example_wasm() -> impl IntoResponse {
//   (
//     [(header::CONTENT_TYPE, "application/wasm")],
//     include_bytes!("../../target/dioxus_web_sys_example_bg.wasm").to_vec(),
//   )
// }
