use leptos::{html::Input, prelude::*};

#[derive(Copy, Clone)]
struct LogContext(RwSignal<Vec<String>>);

#[component]
fn App() -> impl IntoView {
  // Just making a visible log here
  // You can ignore this...
  let log = RwSignal::<Vec<String>>::new(vec![]);
  let logged = move || log.get().join("\n");

  // the newtype pattern isn't *necessary* here but is a good practice
  // it avoids confusion with other possible future `RwSignal<Vec<String>>` contexts
  // and makes it easier to refer to it
  provide_context(LogContext(log));

  view! {
      <CreateAnEffect />
      <pre>{logged}</pre>
  }
}

#[component]
fn CreateAnEffect() -> impl IntoView {
  let (first, set_first) = signal(String::new());
  let (last, set_last) = signal(String::new());
  let (use_last, set_use_last) = signal(true);

  // this will add the name to the log
  // any time one of the source signals changes
  Effect::new(move |_| {
    log(if use_last.get() {
      let first = first.read();
      let last = last.read();
      format!("{first} {last}")
    } else {
      first.get()
    })
  });

  view! {
      <h1>
          <code>"create_effect"</code>
          " Version"
      </h1>
      <form>
          <label>
              "First Name"
              <input
                  type="text"
                  name="first"
                  prop:value=first
                  on:change:target=move |ev| set_first.set(ev.target().value())
              />
          </label>
          <label>
              "Last Name"
              <input
                  type="text"
                  name="last"
                  prop:value=last
                  on:change:target=move |ev| set_last.set(ev.target().value())
              />
          </label>
          <label>
              "Show Last Name"
              <input
                  type="checkbox"
                  name="use_last"
                  prop:checked=use_last
                  on:change:target=move |ev| set_use_last.set(ev.target().checked())
              />
          </label>
      </form>
  }
}

#[component]
fn ManualVersion() -> impl IntoView {
  let first = NodeRef::<Input>::new();
  let last = NodeRef::<Input>::new();
  let use_last = NodeRef::<Input>::new();

  let mut prev_name = String::new();
  let on_change = move |_| {
    log("      listener");
    let first = first.get().unwrap();
    let last = last.get().unwrap();
    let use_last = use_last.get().unwrap();
    let this_one = if use_last.checked() {
      format!("{} {}", first.value(), last.value())
    } else {
      first.value()
    };

    if this_one != prev_name {
      log(&this_one);
      prev_name = this_one;
    }
  };

  view! {
      <h1>"Manual Version"</h1>
      <form on:change=on_change>
          <label>"First Name" <input type="text" name="first" node_ref=first /></label>
          <label>"Last Name" <input type="text" name="last" node_ref=last /></label>
          <label>
              "Show Last Name" <input type="checkbox" name="use_last" checked node_ref=use_last />
          </label>
      </form>
  }
}

fn log(msg: impl std::fmt::Display) {
  let log = use_context::<LogContext>().unwrap().0;
  log.update(|log| log.push(msg.to_string()));
}

fn main() {
  leptos::mount::mount_to_body(App)
}
