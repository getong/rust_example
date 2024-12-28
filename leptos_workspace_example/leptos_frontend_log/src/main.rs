use console_log;
use leptos::prelude::*;

#[component]
fn App() -> impl IntoView {
    let (count, set_count) = signal(0);

  Effect::new(move || {
    log::info!("Current count: {}", count.get());
  });

  view! {
      <button on:click=move |_| {
          *set_count.write() += 1;
      }>{count.get()}</button>
  }
}

fn main() {
  console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");
  console_error_panic_hook::set_once(); // Optional: Better panic messages
  leptos::mount::mount_to_body(App);
}
