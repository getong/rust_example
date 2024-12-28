use leptos::prelude::*;

#[component]
fn App() -> impl IntoView {
  let (count, set_count) = signal(0);

  Effect::new(move || {
    leptos::logging::log!("Current count: {}", count.get());
  });

  view! {
      <button on:click=move |_| {
          *set_count.write() += 1;
      }>{count.get()}</button>
  }
}

fn main() {
  leptos::mount::mount_to_body(App);
}
