use gloo_timers::future::TimeoutFuture;
use leptos::prelude::*;

async fn important_api_call(name: String) -> String {
  TimeoutFuture::new(1_000).await;
  name.to_ascii_uppercase()
}

#[component]
pub fn App() -> impl IntoView {
  let (name, set_name) = signal("Bill".to_string());

  // this will reload every time `name` changes
  let async_data = LocalResource::new(move || important_api_call(name.get()));

  view! {
      <input
          on:change:target=move |ev| {
              set_name.set(ev.target().value());
          }
          prop:value=name
      />
      <p>
          <code>"name:"</code>
          {name}
      </p>
      // the fallback will show whenever a resource
      <Suspense // read "under" the suspense is loading
      fallback=move || view! { <p>"Loading..."</p> }>
          // Suspend allows you use to an async block in the view
          <p>"Your shouting name is " {move || Suspend::new(async move { async_data.await })}</p>
      </Suspense>
      // the fallback will show whenever a resource
      <Suspense // read "under" the suspense is loading
      fallback=move || view! { <p>"Loading..."</p> }>
          // the children will be rendered once initially,
          // and then whenever any resources has been resolved
          <p>
              "Which should be the same as... "
              {move || async_data.get().as_deref().map(ToString::to_string)}
          </p>
      </Suspense>
  }
}

fn main() {
  leptos::mount::mount_to_body(App)
}
