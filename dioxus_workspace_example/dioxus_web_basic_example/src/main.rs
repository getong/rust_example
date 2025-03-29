use dioxus::prelude::*;


pub const ROOT_API_URL: &str = "http://127.0.0.1:8080/";

fn main() {
  // println!("Hello, world!");
  dioxus::launch(app)
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
