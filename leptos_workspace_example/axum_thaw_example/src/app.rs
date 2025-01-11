use chrono::*;
use leptos::prelude::*;
use leptos_meta::{MetaTags, Stylesheet, Title, provide_meta_context};
use leptos_router::{
  StaticSegment,
  components::{Route, Router, Routes},
};
use leptos_use::{
  UseCalendarOptions, UseCalendarReturn, docs::Note, use_active_element, use_calendar_with_options,
};
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
                      <Route path=StaticSegment("leptos_use_calendar") view=UseCalendarDemo />
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
      <p>
          <a href="/leptos_use_calendar">Go to view leptos use calendar</a>
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

#[component]
fn UseCalendarDemo() -> impl IntoView {
  let selected_date = RwSignal::new(Some(Local::now().date_naive()));
  let options = UseCalendarOptions::default()
    .first_day_of_the_week(6)
    .initial_date(selected_date);

  let UseCalendarReturn {
    weekdays,
    dates,
    previous_month,
    today,
    next_month,
  } = use_calendar_with_options(options);

  let current_month_year = Memo::new(move |_| {
    let current = dates
      .get()
      .into_iter()
      .find_map(|date| {
        if !date.is_other_month() && date.is_first_day_of_month() {
          Some(*date)
        } else {
          None
        }
      })
      .unwrap_or(Local::now().date_naive());
    format!(
      "{} {}",
      Month::try_from(current.month() as u8).unwrap().name(),
      current.year(),
    )
  });

  view! {
      <p>
          <a href="/">Back to Home</a>
      </p>
      <div class="w-[50%]">
          <div class="flex center-items justify-between">
              <button on:click=move |_| previous_month()>{"<<"}</button>
              <button on:click=move |_| today()>{"Today"}</button>
              <button on:click=move |_| next_month()>{">>"}</button>
          </div>
          <div class="flex center-items justify-center">{move || current_month_year.get()}</div>
          <div class="grid grid-cols-7">
              {move || {
                  weekdays
                      .get()
                      .iter()
                      .map(|weekday| {
                          view! {
                              <div class="p-1 text-center">
                                  {Weekday::try_from(*weekday as u8).unwrap().to_string()}
                              </div>
                          }
                      })
                      .collect_view()
              }}
              {move || {
                  dates
                      .get()
                      .into_iter()
                      .map(|date| {
                          let is_selected = move || {
                              if let Some(selected_date) = selected_date.get() {
                                  *date == selected_date
                              } else {
                                  false
                              }
                          };
                          view! {
                              <div
                                  class="w-8 h-8 leading-8 cursor-pointer text-center p-4 justify-self-center border-2 border-solid rounded-full"
                                  class:text-red-500=date.is_today()
                                  class:text-gray-500=date.is_other_month()
                                  class:border-red-500=move || is_selected()
                                  class:border-transparent=move || !is_selected()
                                  on:click=move |_| selected_date.set(Some(*date))
                              >

                                  {date.day()}
                              </div>
                          }
                      })
                      .collect_view()
              }}
          </div>
      </div>
  }
}
