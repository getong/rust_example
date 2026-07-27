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

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use blitz::{
    dom::DocumentConfig,
    html::{HtmlDocument, HtmlProvider},
    traits::shell::{ColorScheme, Viewport},
  };

  #[test]
  fn stylesheet_resolves_with_blitz() {
    let html = format!(
      r#"<!doctype html>
        <html>
          <head><style>{}</style></head>
          <body>
            <main class="app-shell">
              <article class="task-row"><p class="task-title">Task</p></article>
            </main>
          </body>
        </html>"#,
      include_str!("../assets/main.css")
    );
    let mut document = HtmlDocument::from_html(
      &html,
      DocumentConfig {
        viewport: Some(Viewport::new(760, 900, 1.0, ColorScheme::Light)),
        html_parser_provider: Some(Arc::new(HtmlProvider) as _),
        ..DocumentConfig::default()
      },
    );

    document.resolve(0.0);

    assert!(
      document
        .query_selector(".task-row")
        .expect("selector should be valid")
        .is_some()
    );
  }
}
