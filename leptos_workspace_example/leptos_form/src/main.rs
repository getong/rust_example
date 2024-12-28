use leptos::prelude::*;
use leptos_router::{
  components::{Form, Route, Router, Routes},
  hooks::use_query_map,
  path,
};

#[component]
pub fn App() -> impl IntoView {
  view! {
      <Router>
          <h1>
              <code>"<Form/>"</code>
          </h1>
          <main>
              <Routes fallback=|| "Not found.">
                  <Route path=path!("") view=FormExample />
              </Routes>
          </main>
      </Router>
  }
}

#[component]
pub fn FormExample() -> impl IntoView {
  // reactive access to URL query
  let query = use_query_map();
  let name = move || query.read().get("name").unwrap_or_default();
  let number = move || query.read().get("number").unwrap_or_default();
  let select = move || query.read().get("select").unwrap_or_default();

  view! {
      // read out the URL query strings
      <table>
          <tr>
              <td>
                  <code>"name"</code>
              </td>
              <td>{name}</td>
          </tr>
          <tr>
              <td>
                  <code>"number"</code>
              </td>
              <td>{number}</td>
          </tr>
          <tr>
              <td>
                  <code>"select"</code>
              </td>
              <td>{select}</td>
          </tr>
      </table>
      // <Form/> will navigate whenever submitted
      <h2>"Manual Submission"</h2>
      <Form method="GET" action="">
          // input names determine query string key
          <input type="text" name="name" value=name />
          <input type="number" name="number" value=number />
          <select name="select">
              // `selected` will set which starts as selected
              <option selected=move || select() == "A">"A"</option>
              <option selected=move || select() == "B">"B"</option>
              <option selected=move || select() == "C">"C"</option>
          </select>
          // submitting should cause a client-side
          // navigation, not a full reload
          <input type="submit" />
      </Form>
      // This <Form/> uses some JavaScript to submit
      // on every input
      <h2>"Automatic Submission"</h2>
      <Form method="GET" action="">
          <input
              type="text"
              name="name"
              value=name
              // this oninput attribute will cause the
              // form to submit on every input to the field
              oninput="this.form.requestSubmit()"
          />
          <input type="number" name="number" value=number oninput="this.form.requestSubmit()" />
          <select name="select" onchange="this.form.requestSubmit()">
              <option selected=move || select() == "A">"A"</option>
              <option selected=move || select() == "B">"B"</option>
              <option selected=move || select() == "C">"C"</option>
          </select>
          // submitting should cause a client-side
          // navigation, not a full reload
          <input type="submit" />
      </Form>
  }
}

fn main() {
  leptos::mount::mount_to_body(App)
}
