use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;

// Here we define an async function
// This could be anything: a network request, database read, etc.
// Here, we just multiply a number by 10
async fn load_data(value: i32) -> i32 {
  // fake a one-second delay
  TimeoutFuture::new(1_000).await;
  value * 10
}

#[component]
pub fn App() -> impl IntoView {
  // this count is our synchronous, local state
  let (count, set_count) = signal(0);

  // tracks `count`, and reloads by calling `load_data`
  // whenever it changes
  let async_data = LocalResource::new(move || load_data(count.get()));

  // a resource will only load once if it doesn't read any reactive data
  let stable = LocalResource::new(|| load_data(1));

  // we can access the resource values with .get()
  // this will reactively return None before the Future has resolved
  // and update to Some(T) when it has resolved
  let async_result = move || {
    async_data
      .get()
      .as_deref()
      .map(|value| format!("Server returned {value:?}"))
      // This loading state will only show before the first load
      .unwrap_or_else(|| "Loading...".into())
  };

  view! {
      <button on:click=move |_| *set_count.write() += 1>"Click me"</button>
      <p>
          <code>"stable"</code>
          ": "
          {move || stable.get().as_deref().copied()}
      </p>
      <p>
          <code>"count"</code>
          ": "
          {count}
      </p>
      <p>
          <code>"async_value"</code>
          ": "
          {async_result}
          <br />
      </p>
  }
}

fn main() {
  leptos::mount::mount_to_body(App)
}
