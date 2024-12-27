use leptos::{mount::mount_to_body, prelude::*};

fn main() {
  mount_to_body(App);
}

/// Shows progress toward a goal.
#[component]
fn ProgressBar(
  // Marks this as an optional prop. It will default to the default
  // value of its type, i.e., 0.
  #[prop(default = 100)]
  /// The maximum value of the progress bar.
  max: u16,
  // Will run `.into()` on the value passed into the prop.
  #[prop(into)]
  // `Signal<T>` is a wrapper for several reactive types.
  // It can be helpful in component APIs like this, where we
  // might want to take any kind of reactive value
  /// How much progress should be displayed.
  progress: Signal<i32>,
) -> impl IntoView {
  view! {
      <progress max=max value=progress />
      <br />
  }
}

#[component]
fn App() -> impl IntoView {
  let (count, set_count) = signal(0);

  let double_count = move || count.get() * 2;

  view! {
      <button on:click=move |_| {
          *set_count.write() += 1;
      }>"Click me"</button>
      <br />
      // If you have this open in CodeSandbox or an editor with
      // rust-analyzer support, try hovering over `ProgressBar`,
      // `max`, or `progress` to see the docs we defined above
      <ProgressBar max=50 progress=count />
      // Let's use the default max value on this one
      // the default is 100, so it should move half as fast
      <ProgressBar progress=count />
      // Signal::derive creates a Signal wrapper from our derived signal
      // using double_count means it should move twice as fast
      <ProgressBar max=50 progress=Signal::derive(double_count) />
  }
}
