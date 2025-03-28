use dioxus::prelude::*;

fn main() {
  dioxus::launch(app);
}

fn app() -> Element {
  let count = use_signal(|| 0);

  rsx! {
      Child { state: count }
  }
}

#[component]
fn Child(state: Signal<u32>) -> Element {
  use_future(move || async move {
    *state.write() += 1;
  });

  rsx! {
      button {
          onclick: move |_| *state.write() += 1,
          "state is {state.read()}"
      }
  }
}
