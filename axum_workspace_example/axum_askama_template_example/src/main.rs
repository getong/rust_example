use askama::Template;
use axum::{response::Html, routing::get, Router};

// Define a template using Askama
#[derive(Template)]
#[template(path = "hello.html")]
struct HelloTemplate<'a> {
  name: &'a str,
}

// Handler function to render the template
async fn hello_handler() -> Html<String> {
  let template = HelloTemplate {
    name: "World! This is from askama template",
  };

  Html(template.render().unwrap())
}

#[tokio::main]
async fn main() {
  // Build our application with a route
  let app = Router::new().route("/", get(hello_handler));

  println!("http://localhost:3000 to view askama template.");

  // Run our application
  let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

  axum::serve(listener, app).await.unwrap();
}
