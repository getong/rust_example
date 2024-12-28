use leptos::prelude::*;
use leptos_router::{
  components::{A, Outlet, ParentRoute, Route, Router, Routes},
  hooks::use_params_map,
  path,
};

#[component]
pub fn App() -> impl IntoView {
  view! {
      <Router>
          <h1>"Contact App"</h1>
          // this <nav> will show on every routes,
          // because it's outside the <Routes/>
          // note: we can just use normal <a> tags
          // and the router will use client-side navigation
          <nav>
              <a href="/">"Home"</a>
              <a href="/contacts">"Contacts"</a>
          </nav>
          <main>
              <Routes fallback=|| "Not found.">
                  // / just has an un-nested "Home"
                  <Route path=path!("/") view=|| view! { <h3>"Home"</h3> } />
                  // /contacts has nested routes
                  <ParentRoute path=path!("/contacts") view=ContactList>
                      // if no id specified, fall back
                      <ParentRoute path=path!(":id") view=ContactInfo>
                          <Route
                              path=path!("")
                              view=|| view! { <div class="tab">"(Contact Info)"</div> }
                          />
                          <Route
                              path=path!("conversations")
                              view=|| view! { <div class="tab">"(Conversations)"</div> }
                          />
                      </ParentRoute>
                      // if no id specified, fall back
                      <Route
                          path=path!("")
                          view=|| {
                              view! {
                                  <div class="select-user">
                                      "Select a user to view contact info."
                                  </div>
                              }
                          }
                      />
                  </ParentRoute>
              </Routes>
          </main>
      </Router>
  }
}

#[component]
fn ContactList() -> impl IntoView {
  view! {
      <div class="contact-list">
          // here's our contact list component itself
          <h3>"Contacts"</h3>
          <div class="contact-list-contacts">
              <A href="alice">"Alice"</A>
              <A href="bob">"Bob"</A>
              <A href="steve">"Steve"</A>
          </div>

          // <Outlet/> will show the nested child route
          // we can position this outlet wherever we want
          // within the layout
          <Outlet />
      </div>
  }
}

#[component]
fn ContactInfo() -> impl IntoView {
  // we can access the :id param reactively with `use_params_map`
  let params = use_params_map();
  let id = move || params.read().get("id").unwrap_or_default();

  // imagine we're loading data from an API here
  let name = move || match id().as_str() {
    "alice" => "Alice",
    "bob" => "Bob",
    "steve" => "Steve",
    _ => "User not found.",
  };

  view! {
      <h4>{name}</h4>
      <div class="contact-info">
          <div class="tabs">
              <A href="" exact=true>
                  "Contact Info"
              </A>
              <A href="conversations">"Conversations"</A>
          </div>

          // <Outlet/> here is the tabs that are nested
          // underneath the /contacts/:id route
          <Outlet />
      </div>
  }
}

fn main() {
  leptos::mount::mount_to_body(App)
}
