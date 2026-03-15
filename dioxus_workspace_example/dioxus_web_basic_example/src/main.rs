use dioxus::prelude::*;

pub const ROOT_API_URL: &str = "http://127.0.0.1:8080/";

#[cfg(target_arch = "wasm32")]
fn main() {
  dioxus::launch(app)
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  dioxus_desktop::launch::launch(
    app,
    Vec::<Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync>>::new(),
    Vec::<Box<dyn std::any::Any>>::new(),
  );
}

pub fn app() -> Element {
  rsx! {
      div {"hello, world!"}
  }
}

// cargo install dioxus-cli
// rustup target add wasm32-unknown-unknown
// dx serve
// copy from https://dioxuslabs.com/learn/0.4/getting_started/wasm
