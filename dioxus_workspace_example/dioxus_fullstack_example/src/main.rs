#![allow(non_snake_case, unused)]
use dioxus::prelude::*;
use dioxus_fullstack::prelude::*;

fn main() {
  LaunchBuilder::new(app).launch();
}

fn app(cx: Scope) -> Element {
  let mut count = use_state(cx, || 0);

  cx.render(rsx! {
      h1 { "High-Five counter: {count}" }
      button { onclick: move |_| count += 1, "Up high!" }
      button { onclick: move |_| count -= 1, "Down low!" }
  })
}

// copy from https://dioxuslabs.com/learn/0.4/getting_started/fullstack
// dx build --features web --release
// cargo run --features ssr --release

// dx build --features web
// dx serve --features ssr --hot-reload --platform desktop
