//! Run with:
//!
//! ```sh
//! dx serve --platform web
//! ```

#![allow(non_snake_case, unused)]
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

fn app() -> Element {
  let mut count = use_signal(|| 0);
  let mut text = use_signal(|| "...".to_string());
  let server_future = use_server_future(get_server_data)?;

  rsx! {
      document::Link { href: asset!("/assets/hello.css"), rel: "stylesheet" }
      h1 { "High-Five counter: {count}" }
      button { onclick: move |_| log_hello(), "click" }
      button { onclick: move |_| count += 1, "Up high!" }
      button { onclick: move |_| count -= 1, "Down low!" }
      button {
          onclick: move |_| async move {
              if let Ok(data) = get_server_data().await {
                  println!("Client received: {}", data);
                  text.set(data.clone());
                  post_server_data(data).await.unwrap();
              }
          },
          "Run a server function!"
      }
      "Server said: {text}"
  }
}

#[server]
async fn post_server_data(data: String) -> Result<(), ServerFnError> {
  println!("Server received: {}", data);

  Ok(())
}

#[server]
async fn get_server_data() -> Result<String, ServerFnError> {
  Ok(reqwest::get("https://httpbin.org/ip").await?.text().await?)
}

#[cfg(target_arch = "wasm32")]
fn main() {
  dioxus::launch(app);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  configure_native_server_url();
  dioxus_desktop::launch::launch(
    app,
    Vec::<Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync>>::new(),
    Vec::<Box<dyn std::any::Any>>::new(),
  );
}

#[cfg(not(target_arch = "wasm32"))]
fn configure_native_server_url() {
  if dioxus::fullstack::get_server_url().is_empty() {
    let server_url = format!(
      "http://{}:{}",
      std::env::var("DIOXUS_DEVSERVER_IP").unwrap_or_else(|_| "127.0.0.1".to_string()),
      std::env::var("DIOXUS_DEVSERVER_PORT").unwrap_or_else(|_| "8080".to_string()),
    );

    dioxus::fullstack::set_server_url(server_url.leak());
  }
}

#[cfg(target_arch = "wasm32")]
fn log_hello() {
  web_sys::console::log_1(&"hello world".into());
}

#[cfg(not(target_arch = "wasm32"))]
fn log_hello() {
  println!("hello world");
}
