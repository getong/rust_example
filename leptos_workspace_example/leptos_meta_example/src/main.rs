use leptos::prelude::*;
use leptos_meta::*;

#[component]
fn MyApp() -> impl IntoView {
  // Provides a [`MetaContext`], if there is not already one provided.
  provide_meta_context();

  let (name, set_name) = signal("Alice".to_string());

  view! {
      <Title
          // reactively sets document.title when `name` changes
          text=move || name.get()
          // applies the `formatter` function to the `text` value
          formatter=|text| format!("“{text}” is your name")
      />
      <main>
          <input
              prop:value=move || name.get()
              on:input=move |ev| set_name.set(event_target_value(&ev))
          />
      </main>
  }
}

fn main() {
  leptos::mount::mount_to_body(MyApp);
}
