use leptos::{prelude::*, wasm_bindgen::JsCast};
use leptos_meta::{provide_meta_context, MetaTags, Stylesheet, Title};
use leptos_router::{
  components::{Route, Router, Routes},
  StaticSegment,
};
use leptos_use::{docs::Note, use_active_element};

pub fn shell(options: LeptosOptions) -> impl IntoView {
  view! {
      <!DOCTYPE html>
      <html lang="en">
          <head>
              <meta charset="utf-8" />
              <meta name="viewport" content="width=device-width, initial-scale=1" />
              <AutoReload options=options.clone() />
              <HydrationScripts options />
              <MetaTags />
          </head>
          <body>
              <App />
          </body>
      </html>
  }
}

#[component]
pub fn App() -> impl IntoView {
  // Provides context that manages stylesheets, titles, meta tags, etc.
  provide_meta_context();

  view! {
      // injects a stylesheet into the document <head>
      // id=leptos means cargo-leptos will hot-reload this stylesheet
      <Stylesheet id="leptos" href="/pkg/axum-leptos-example.css" />

      // sets the document title
      <Title text="Welcome to Leptos" />

      // content for this welcome page
      <Router>
          <main>
              <Routes fallback=|| "Page not found.".into_view()>
                  <Route path=StaticSegment("") view=HomePage />
                  <Route path=StaticSegment("hello") view=HelloWorld />
                  <Route path=StaticSegment("leptos_use_active_element") view=ActiveElementDemo />
              </Routes>
          </main>
      </Router>
  }
}

/// Renders the home page of your application.
#[component]
fn HomePage() -> impl IntoView {
  // Creates a reactive value to update the button
  let count = RwSignal::new(0);
  let on_click = move |_| *count.write() += 1;

  view! {
      <h1>"Welcome to Leptos!"</h1>
      <button on:click=on_click>"Click Me: " {count}</button>
      <p>
          <a href="/hello">Go to Hello World page</a>
      </p>
      <p>
          <a href="/leptos_use_active_element">Go to leptos use element page</a>
      </p>
  }
}

/// A new component that renders "Hello World" and a button to go back to the homepage.
#[component]
fn HelloWorld() -> impl IntoView {
  view! {
      <h1>"Hello, World!"</h1>
      <p>
          <a href="/">Back to Home</a>
      </p>
  }
}

#[component]
fn ActiveElementDemo() -> impl IntoView {
  let active_element = use_active_element();
  let key = move || {
    format!(
      "{:?}",
      active_element
        .get()
        .map(|el| el
          .unchecked_ref::<web_sys::HtmlElement>()
          .dataset()
          .get("id"))
        .unwrap_or_default()
    )
  };

  view! {
      <Note class="mb-3">"Select the inputs below to see the changes"</Note>

      <div class="grid grid-cols-1 md:grid-cols-3 gap-2">
          <For each=move || (1..7) key=|i| *i let:i>
              <input type="text" data-id=i class="!my-0 !min-w-0" placeholder=i />
          </For>

      </div>

      <div class="mt-2">"Current Active Element: " <span class="text-primary">{key}</span></div>
      <p>
          <a href="/">Back to Home</a>
      </p>
  }
}
