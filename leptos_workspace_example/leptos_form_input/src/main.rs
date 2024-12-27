use leptos::{ev::SubmitEvent, prelude::*};

#[component]
fn App() -> impl IntoView {
  view! {
      <h2>"Controlled Component"</h2>
      <ControlledComponent />
      <h2>"Uncontrolled Component"</h2>
      <UncontrolledComponent />
  }
}

#[component]
fn ControlledComponent() -> impl IntoView {
  // create a signal to hold the value
  let (name, set_name) = signal("Controlled".to_string());

  view! {
      <input
          type="text"
          // fire an event whenever the input changes
          // adding :target after the event gives us access to
          // a correctly-typed element at ev.target()
          on:input:target=move |ev| {
              set_name.set(ev.target().value());
          }

          // the `prop:` syntax lets you update a DOM property,
          // rather than an attribute.
          // 
          // IMPORTANT: the `value` *attribute* only sets the
          // initial value, until you have made a change.
          // The `value` *property* sets the current value.
          // This is a quirk of the DOM; I didn't invent it.
          // Other frameworks gloss this over; I think it's
          // more important to give you access to the browser
          // as it really works.
          // 
          // tl;dr: use prop:value for form inputs
          prop:value=name
      />
      <p>"Name is: " {name}</p>
  }
}

#[component]
fn UncontrolledComponent() -> impl IntoView {
  // import the type for <input>
  use leptos::html::Input;

  let (name, set_name) = signal("Uncontrolled".to_string());

  // we'll use a NodeRef to store a reference to the input element
  // this will be filled when the element is created
  let input_element: NodeRef<Input> = NodeRef::new();

  // fires when the form `submit` event happens
  // this will store the value of the <input> in our signal
  let on_submit = move |ev: SubmitEvent| {
    // stop the page from reloading!
    ev.prevent_default();

    // here, we'll extract the value from the input
    let value = input_element
      .get()
      // event handlers can only fire after the view
      // is mounted to the DOM, so the `NodeRef` will be `Some`
      .expect("<input> to exist")
      // `NodeRef` implements `Deref` for the DOM element type
      // this means we can call`HtmlInputElement::value()`
      // to get the current value of the input
      .value();
    set_name.set(value);
  };

  view! {
      <form on:submit=on_submit>
          <input
              type="text"
              // here, we use the `value` *attribute* to set only
              // the initial value, letting the browser maintain
              // the state after that
              value=name

              // store a reference to this input in `input_element`
              node_ref=input_element
          />
          <input type="submit" value="Submit" />
      </form>
      <p>"Name is: " {name}</p>
  }
}

// This `main` function is the entry point into the app
// It just mounts our component to the <body>
// Because we defined it as `fn App`, we can now use it in a
// template as <App/>
fn main() {
  leptos::mount::mount_to_body(App)
}
