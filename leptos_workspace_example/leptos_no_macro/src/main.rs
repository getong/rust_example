use leptos::{
  ev,
  html::{button, div, span},
  prelude::*,
};

/// A simple counter view.
// A component is really just a function call: it runs once to create the DOM and reactive system
pub fn counter(initial_value: i32, step: i32) -> impl IntoView {
  let (count, set_count) = signal(initial_value);
  div().child((
    button()
      // typed events found in leptos::ev
      // 1) prevent typos in event names
      // 2) allow for correct type inference in callbacks
      .on(ev::click, move |_| set_count.set(0))
      .child("Clear"),
    button()
      .on(ev::click, move |_| *set_count.write() -= step)
      .child("-1"),
    span().child(("Value: ", move || count.get(), "!")),
    button()
      .on(ev::click, move |_| *set_count.write() += step)
      .child("+1"),
  ))
}

fn main() {
  leptos::mount::mount_to_body(|| counter(2, 3))
}
