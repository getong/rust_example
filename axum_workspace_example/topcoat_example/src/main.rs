use topcoat::{
  Result,
  router::{Router, RouterBuilderDiscoverExt, page},
  view::{View, component, view},
};

#[tokio::main]
async fn main() {
  topcoat::start(Router::builder().discover().build())
    .await
    .unwrap();
}

#[page("/")]
async fn home() -> Result<impl View> {
  Ok(view! {
    <!DOCTYPE html>
      <html>
      <body>
      hello(name: "World")
      </body>
      </html>
  })
}

#[component]
async fn hello(name: &str) -> Result<impl View> {
  Ok(view! { <h1>"Hello, " (name) "!"</h1> })
}
