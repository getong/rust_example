use components::Navbar;
use dioxus::prelude::*;
use views::{Blog, Home, JsSample};

mod components;
mod views;

#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Navbar)]
    #[route("/")]
    Home {},
    #[route("/blog/:id")]
    Blog { id: i32 },
    #[route("/js_sample")]
    JsSample {},
}

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/styling/main.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

#[cfg(target_arch = "wasm32")]
fn main() {
  dioxus::launch(App);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  configure_native_server_url();
  dioxus_desktop::launch::launch(
    App,
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

#[component]
fn App() -> Element {
  // Build cool things ✌️

  rsx! {
      // Global app resources
      document::Link { rel: "icon", href: FAVICON }
      document::Link { rel: "stylesheet", href: MAIN_CSS }
      document::Link { rel: "stylesheet", href: TAILWIND_CSS }
      document::Stylesheet { href: MAIN_CSS }
      Router::<Route> {}
  }
}
