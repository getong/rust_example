use dioxus::prelude::*;

#[cfg(target_arch = "wasm32")]
fn main() {
  dioxus::launch(app);
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
  dioxus_desktop::launch::launch(
    app,
    Vec::<Box<dyn Fn() -> Box<dyn std::any::Any> + Send + Sync>>::new(),
    Vec::<Box<dyn std::any::Any>>::new(),
  );
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
