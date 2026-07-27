use dioxus::prelude::*;

use crate::features::tasks::{TaskBloc, TaskRoute, default_database_path};

const MAIN_CSS: Asset = asset!("/assets/main.css");

#[component]
pub fn App() -> Element {
  use_context_provider(|| TaskBloc::open(default_database_path()));

  rsx! {
    document::Title { "Focus Board" }
    document::Stylesheet { href: MAIN_CSS }
    Router::<TaskRoute> {}
  }
}
