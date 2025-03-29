use axum::{Router, response::Html, routing::get};
use dioxus::prelude::*;
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
