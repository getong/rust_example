#![allow(non_snake_case)]
use dioxus::prelude::*;

// The entry point for the server
#[cfg(feature = "server")]
#[tokio::main]
async fn main() {
  // Get the address the server should run on. If the CLI is running, the CLI proxies fullstack into
  // the main address and we use the generated address the CLI gives us
  let address = dioxus::cli_config::fullstack_address_or_localhost();

  // Set up the axum router
  let router = axum::Router::new()
    // You can add a dioxus application to the router with the `serve_dioxus_application` method
    // This will add a fallback route to the router that will serve your component and server
    // functions
    .serve_dioxus_application(ServeConfigBuilder::default(), App);

  // Finally, we can launch the server
  let router = router.into_make_service();
  let listener = tokio::net::TcpListener::bind(address).await.unwrap();
  axum::serve(listener, router).await.unwrap();
}

// For any other platform, we just launch the app
#[cfg(not(feature = "server"))]
fn main() {
  dioxus::launch(App);
}

#[component]
fn App() -> Element {
  let mut meaning = use_signal(|| None);

  rsx! {
      h1 { "Meaning of life: {meaning:?}" }
      button {
          onclick: move |_| async move {
              if let Ok(data) = get_meaning("life the universe and everything".into()).await {
                  meaning.set(data);
              }
          },
          "Run a server function"
      }
  }
}

#[server]
async fn get_meaning(of: String) -> Result<Option<u32>, ServerFnError> {
  Ok(of.contains("life").then(|| 42))
}

// copy from https://docs.rs/dioxus-fullstack/0.6.3/dioxus_fullstack/#axum-integration
// bug , see https://github.com/DioxusLabs/dioxus/issues/3790
// also see https://github.com/DioxusLabs/dioxus/pull/3693