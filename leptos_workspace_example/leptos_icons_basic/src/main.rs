use leptos::prelude::*;
use leptos_icons::Icon;

fn main() {
  leptos::mount::mount_to_body(|| {
    view! {
        <Icon icon=icondata::BsFolder />
    }
  })
}
