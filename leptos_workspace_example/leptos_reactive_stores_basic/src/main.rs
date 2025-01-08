use leptos::prelude::*;
use leptos_reactive_stores_basic::App;

pub fn main() {
  console_error_panic_hook::set_once();
  mount_to_body(App)
}
