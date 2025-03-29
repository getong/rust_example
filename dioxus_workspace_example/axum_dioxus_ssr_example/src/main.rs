use axum::{Router, response::Html, routing::get};
use dioxus::prelude::*;

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
      .into_make_service(),
  )
  .await
  .unwrap();
}

// async fn app_endpoint() -> Html<String> {
//     // render the rsx! macro to HTML
//     Html(dioxus_ssr::render_element(rsx! { div { "hello world!" } }))
// }

fn app() -> Element {
  rsx! { div { "hello world" } }
}

async fn app_endpoint() -> Html<String> {
  // create a component that renders a div with the text "hello world"

  // create a VirtualDom with the app component
  let mut app = VirtualDom::new(app);
  // rebuild the VirtualDom before rendering
  app.rebuild_in_place();

  // render the VirtualDom to HTML
  Html(dioxus_ssr::render(&app))
}
