use leptos::prelude::*;
use leptos_use::{UseMouseReturn, use_mouse};

#[component]
fn Demo() -> impl IntoView {
  let UseMouseReturn { x, y, .. } = use_mouse();

  view! {
      "x: "
      {x}
      " y: "
      {y}
  }
}

fn main() {
  leptos::mount::mount_to_body(Demo)
}
