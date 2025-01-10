use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
  StaticSegment,
  components::{Route, Router, Routes},
};
use leptos_use::{docs::Note, use_active_element};
use thaw::{Button, Calendar, ConfigProvider, Space, ssr::SSRMountStyleProvider};
use wasm_bindgen::JsCast;

pub fn shell(options: LeptosOptions) -> impl IntoView {
  view! {
      <SSRMountStyleProvider>
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
      </SSRMountStyleProvider>
  }
}

#[component]
pub fn App() -> impl IntoView {
  // Provides context that manages stylesheets, titles, meta tags, etc.
  provide_meta_context();

  view! {
      // injects a stylesheet into the document <head>
      // id=leptos means cargo-leptos will hot-reload this stylesheet
      <Stylesheet id="leptos" href="/pkg/axum_thaw_example.css" />

      // sets the document title
      <Title text="Welcome to Leptos" />

      // content for this welcome page
      <ConfigProvider>
          <Router>
              <main>
                  <Routes fallback=|| "Page not found.".into_view()>
                      <Route path=StaticSegment("") view=HomePage />
                      <Route path=StaticSegment("hello") view=HelloWorld />
                      <Route
                          path=StaticSegment("leptos_use_active_element")
                          view=ActiveElementDemo
                      />
                      <Route path=StaticSegment("calendar") view=CalendarElement />
                  </Routes>
              </main>
          </Router>
      </ConfigProvider>
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
      <Button on_click=on_click>"Click Me: " {count}</Button>
      <p>
          <a href="/hello">Go to Hello World page</a>
      </p>
      <p>
          <a href="/leptos_use_active_element">Go to leptos use element page</a>
      </p>
      <p>
          <a href="/calendar">Go to view calendar</a>
      </p>
  }
}

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

#[component]
fn CalendarElement() -> impl IntoView {
  use chrono::prelude::*;
  let value = RwSignal::new(Local::now().date_naive());
  let option_value = RwSignal::new(Some(Local::now().date_naive()));

  view! {
      <Space vertical=true>
          <Calendar value />
          <Calendar value=option_value let(date: &NaiveDate)>
              {date.year()}
              "-"
              {date.month()}
              "-"
              {date.day()}
          </Calendar>
      </Space>
      <p>
          <a href="/">Back to Home</a>
      </p>
  }
}
